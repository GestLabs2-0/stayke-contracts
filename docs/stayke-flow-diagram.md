# 📊 Diagramas de Flujo — Stayke Contracts

> Diagramas del estado actual de los 5 contratos Anchor, sus relaciones CPI y ciclos de vida.
> Generado con Mermaid.js — renderizable en GitHub, VS Code y cualquier visor Markdown.

---

## 📋 Convenciones

| Símbolo | Significado |
|---------|-------------|
| `[Usuario]` | Acción del usuario (firma con su wallet) |
| `[Admin]` | Acción restringida a authority/admin |
| `{CPI}` | Cross-Program Invocation |
| `{Lectura}` | Lectura de estado sin mutar |
| `##` | Estado intermedio de cuenta |
| `==X==>` | Transición con condición sobre el estado actual |
| `- - ->` | Flujo secundario / off-chain |
| `close ✗` | La cuenta Booking se cierra (renta devuelta) |

---

## 1. 🌍 Vista General del Ecosistema

```mermaid
flowchart LR
    subgraph "⚙️ stayke-config"
        GC[GlobalConfig<br/>authority · fee_bps · min_deposit<br/>usdc_mint · platform_vault]
    end

    subgraph "🧠 stayke-core"
        CP[ConfigAcc]
        UP[UserProfile]
        RP[ReputationProfile]
        ID[Identity]
        LI[Listing]
    end

    subgraph "🏦 stayke-escrow"
        EC[EscrowConfig]
        BK[Booking]
        BD[BookingDays]
    end

    subgraph "⚖️ stayke-disputes"
        DC[DisputeConfig]
        DS[Dispute]
    end

    subgraph "💰 stayke-treasury"
        TC[TreasuryConfig]
        TV[(Treasury Vault)]
    end

    GC -.->|Lectura: fee_bps, usdc_mint| BK
    GC -.->|Lectura: usdc_mint, min_deposit| TV
    GC -.->|Lectura: usdc_mint| DS

    UP -.->|Lectura: is_verified, banned, deposited| BK
    LI -.->|Lectura: price, is_occupied| BK

    DS -->|{CPI} cpi_update_booking_status| BK
    DS -->|{CPI} cpi_resolve_dispute_transfer| BK
    DS -->|{CPI} add_infraction| RP
    DS -->|{CPI} clear_active_booking| UP
    DS -->|{CPI} clear_listing_booking| LI
    DS --x|{CPI} cpi_penalize_transfer (TODO)| TV

    TV -->|{CPI} update_deposit| UP
```

---

## 2. 🔄 Ciclo de Vida de una Reserva (Booking)

> Basado en el código real en `programs/stayke-escrow/src/instructions/booking.rs` y `dispute_cpi.rs`.

```mermaid
stateDiagram-v2
    [*] --> Pending: create_booking (guest)
    
    Pending --> HostAccepted: host_accept_booking (host)
    Pending --> Cancelled: host_reject_booking (host)  close✗
    Pending --> Cancelled: client_reject_reserve (guest)  close✗
    
    HostAccepted --> Active: client_accept_reserve (guest)\nTransfiere USDC a escrow vault\nListing.is_occupied = guest
    HostAccepted --> Cancelled: client_reject_reserve (guest)  close✗
    
    Active --> ReviewCompleted: review_completed (guest)\nScore 1-5 · Actualiza ReputationProfile
    
    Active --> Disputed: open_dispute (guest/host)\n{CPI} → Escrow: cpi_update_booking_status(Disputed)
    
    ReviewCompleted --> Completed: complete_stay\nDistribuye USDC al host + fee platform\nCierra escrow token account  close✗
    
    Disputed --> DisputeResolved: resolve_dispute\n{CPI} → Escrow: cpi_resolve_dispute_transfer\nReparte fondos, cierra escrow
    Disputed --> DisputeRejected: resolve_dispute (rejected=true)\n{CPI} → Escrow: cpi_resolve_dispute_transfer\nFondos devueltos al guest

    note right of DisputeResolved
        close_dispute (admin) →
        {CPI} clear_active_booking (guest)
        {CPI} clear_active_booking (host)
        {CPI} clear_listing_booking
        Cierra la cuenta Dispute
    end

    note right of DisputeRejected
        close_dispute (admin) →
        mismo cleanup que Resolved
    end
    
    Completed --> [*]
    Cancelled --> [*]
    DisputeResolved --> [*]
    DisputeRejected --> [*]
```

### Diagrama de Secuencia — Reserva Feliz (sin disputa)

```mermaid
sequenceDiagram
    participant G as Guest
    participant H as Host
    participant E as stayke-escrow
    participant C as stayke-core
    participant CF as stayke-config

    G->>E: create_booking(check_in, check_out)
    E->>C: {Lectura} UserProfile(guest): verified, banned, deposited
    E->>C: {Lectura} UserProfile(host): verified, banned, deposited
    E->>C: {Lectura} Listing: price, is_occupied
    E->>CF: {Lectura} GlobalConfig: fee_bps, usdc_mint, min_deposit
    E-->>G: Booking → Pending

    H->>E: host_accept_booking
    Note over E: require status == Pending
    E-->>H: Booking → HostAccepted

    G->>E: client_accept_reserve
    Note over E: require status == HostAccepted
    E->>E: Transfiere USDC a escrow vault
    E->>C: Listing.is_occupied = guest (lectura directa)
    E-->>G: Booking → Active

    Note over G,H: Estancia transcurre...

    G->>E: review_completed(score)
    Note over E: require status == Active<br/>score entre 1 y 5
    E->>E: ReputationProfile.host_reviews += 1
    E->>E: ReputationProfile.total_score_host += score
    E->>E: ReputationProfile.hosted_stays += 1
    E-->>G: Booking → ReviewCompleted

    H->>E: complete_stay
    Note over E: require status == ReviewCompleted
    E->>E: fee = total_price * fee_bps / 10000
    E->>E: host_amount = total_price - fee
    E->>H: Transfiere host_amount (desde escrow)
    E->>CF: Transfiere fee a platform_vault
    E->>E: Cierra escrow token account
    Note over E: Booking account se cierra (close=client)
    E-->>H: Booking → Completed
```

---

## 3. ⚖️ Flujo de Disputa (Máxima Interconexión)

```mermaid
sequenceDiagram
    participant U as Initiator (Guest/Host)
    participant D as stayke-disputes
    participant E as stayke-escrow
    participant C as stayke-core
    participant T as stayke-treasury
    participant A as Admin

    U->>D: open_dispute(reason)
    Note over D: Valida que sea guest o host del booking
    D->>E: {CPI} cpi_update_booking_status(Disputed)
    Note over E: require status == Active
    D-->>U: Dispute → Open, Booking → Disputed

    Note over A: Análisis off-chain por el equipo

    A->>D: resolve_dispute(host_share_bps, rejected)
    Note over D: Solo admin configurado en DisputeConfig
    
    D->>E: {CPI} cpi_resolve_dispute_transfer(host_share_bps, rejected)
    Note over E: require status == Disputed
    alt rejected == false
        E->>E: host_amount = distributable * host_share_bps / 10000
        E->>E: guest_amount = distributable - host_amount
        E->>H: Transfiere host_amount
        E->>G: Transfiere guest_amount
    else rejected == true
        E->>G: Transfiere todo (distributable) al guest
    end
    E->>CF: Transfiere fee a platform_vault
    E->>E: Cierra escrow token account
    E-->>A: Booking → DisputeResolved/DisputeRejected

    D-->>A: Dispute → Resolved/Rejected

    A->>D: close_dispute
    Note over D: Solo admin
    D->>C: {CPI} clear_active_booking (guest)
    D->>C: {CPI} clear_active_booking (host)
    D->>C: {CPI} clear_listing_booking (Listing.is_occupied = None)
    Note over D: Se cierra la cuenta Dispute
    D-->>A: Dispute closed
```

### Máquina de Estados de Disputa

```mermaid
stateDiagram-v2
    [*] --> Open: open_dispute
    
    Open --> Resolved: resolve_dispute (rejected=false)\n{CPI} Escrow: reparte fondos<br/>{CPI} Core: add_infraction (queda como TODO en admin.rs)
    
    Open --> Rejected: resolve_dispute (rejected=true)\n{CPI} Escrow: devuelve todo al guest
    
    Resolved --> [*]: close_dispute\n{CPI} Core: clear_active_booking x2\n{CPI} Core: clear_listing_booking
    Rejected --> [*]: close_dispute\n{CPI} Core: mismo cleanup
```

---

## 4. 💰 Flujo de Garantías (Treasury)

```mermaid
sequenceDiagram
    participant U as User
    participant T as stayke-treasury
    participant C as stayke-core
    participant CF as stayke-config

    Note over U: DEPOSITAR GARANTÍA

    U->>T: deposit_guarantee(amount)
    T->>CF: {Lectura} GlobalConfig: usdc_mint, min_deposit
    T->>T: Transfiere USDC user → treasury_vault
    T->>C: {CPI} update_deposit(amount, is_deposit=true)
    T-->>U: UserProfile.deposited += amount

    Note over U: RETIRAR GARANTÍA

    U->>T: withdraw_guarantee(amount)
    T->>CF: {Lectura} GlobalConfig: usdc_mint
    T->>C: {Lectura} UserProfile: deposited, active_booking
    T->>T: Transfiere USDC treasury_vault → user
    T->>C: {CPI} update_deposit(amount, is_deposit=false)
    T-->>U: UserProfile.deposited -= amount
```

---

## 5. 🆔 Flujo de Onboarding de Usuario

```mermaid
flowchart TD
    U[Usuario] -->|1. initialize_user_profile| CORE[stayke-core]
    CORE --> C{Crea 3 PDAs}
    C --> UP[UserProfile<br/>owner, deposited=0, is_verified=false]
    C --> ID[Identity<br/>id, country_code, doctype, is_banned=false]
    C --> RP[ReputationProfile<br/>scores=0, infractions=0]
    
    UP --> G{is_verified?}
    G -->|false| H[Off-chain KYC/KYB]
    H --> AD[Admin: verify_identity]
    AD -->|Solo authority del ConfigAcc| V[Identity.verified_at = now<br/>is_verified = true]
    
    G -->|true| V
    
    V --> DEP[2. deposit_guarantee]
    DEP --> TRE[stayke-treasury]
    TRE --> TR[Transfiere USDC a treasury_vault]
    TR --> CPI[CPI → Core: update_deposit]
    CPI --> ACT[Usuario activo ✅]
    
    ACT --> P{Puede...}
    P --> L[Crear Listings\ncomo host]
    P --> B[Reservar propiedades\ncomo guest]
    
    L -->|initialize_listing| LI[Listing<br/>price, state_hash]
    B -->|create_booking| BK[Booking]
```

---

## 6. 🗺️ Mapa de Relaciones Entre Programas

```mermaid
flowchart TD
    subgraph Lecturas
        CF[stayke-config<br/>GlobalConfig] -.->|fee_bps, usdc_mint,<br/>platform_vault, min_deposit| ESC
        CF -.->|usdc_mint, min_deposit| TRE[stayke-treasury]
        CF -.->|usdc_mint| DIS[stayke-disputes]
        ESC -.->|UserProfile: verified,<br/>banned, deposited| COR[stayke-core]
        ESC -.->|Listing: price,<br/>is_occupied| COR
        DIS -.->|UserProfile| COR
        TRE -.->|UserProfile: deposited| COR
    end

    subgraph CPIs
        TRE -->|update_deposit| COR
        DIS -->|cpi_update_booking_status| ESC
        DIS -->|cpi_resolve_dispute_transfer| ESC
        DIS -->|add_infraction| COR
        DIS -->|clear_active_booking| COR
        DIS -->|clear_listing_booking| COR
        DIS -.->|cpi_penalize_transfer<br/>TODO| TRE
    end

    subgraph "Escritura directa (sin CPI)"
        ESC -->|review_completed| RP[ReputationProfile<br/>host_reviews, scores]
    end
```

---

## Leyenda de Estados Booking

| Estado | Descripción | Transiciones válidas desde código |
|--------|-------------|-----------------------------------|
| `Pending` | Creada por guest, esperando respuesta del host | → HostAccepted, Cancelled |
| `HostAccepted` | Host aceptó, esperando que guest confirme | → Active, Cancelled |
| `Active` | Guest confirmó, fondos en escrow, estancia activa | → ReviewCompleted, Disputed |
| `ReviewCompleted` | Guest calificó (1-5), esperando complete_stay | → Completed |
| `Completed` | Finalizada, fondos distribuidos, booking cerrado | Terminal |
| `Cancelled` | Cancelada, booking cerrado (close) | Terminal |
| `Disputed` | Disputa abierta, booking congelado | → DisputeResolved, DisputeRejected |
| `DisputeResolved` | Disputa resultta, fondos repartidos | Terminal (close_dispute solo cleanup en Core) |
| `DisputeRejected` | Disputa rechazada, fondos devueltos | Terminal (close_dispute solo cleanup en Core) |

### Restricciones por instrucción

| Instrucción | Estado Requerido | Handler |
|-------------|------------------|---------|
| `host_accept_booking` | `Pending` | `handler_host_accept_booking` |
| `host_reject_booking` | `Pending` (close booking) | `handler_host_reject_booking` |
| `client_accept_reserve` | `HostAccepted` | `handler_client_accept_reserve` |
| `client_reject_reserve` | `HostAccepted` o `Pending` (close booking) | `handler_client_reject_reserve` |
| `review_completed` | `Active` | `handler_review_completed` |
| `complete_stay` | `ReviewCompleted` | `handler_complete_stay` |
| `open_dispute` | `Active` (via CPI a Escrow) | `handler_open_dispute` |
| `resolve_dispute` | `Disputed` (via CPI a Escrow) | `handler_resolve_dispute` |

---

> **Nota**: Este diagrama refleja el código actual en la rama `STK-97-re-organizar-instrucciones-en-stayke-treasury`.
> Basado en `booking.rs` (823 líneas), `dispute_cpi.rs` (240 líneas) y `manage_disputes.rs` (288 líneas).
> Los TODOs conocidos (cpi_penalize_transfer, withdraw fees, validaciones faltantes) están marcados con `TODO`.
