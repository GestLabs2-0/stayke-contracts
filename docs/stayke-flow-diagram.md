# Diagramas de flujo — Stayke Contracts (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Mermaid alineado a `lib.rs` / CPI actuales. Referencia de cuentas: [stayke-global.guide.md](./stayke-global.guide.md). Detalle de instrucciones: [disputes](./stayke-disputes.guide.md) y [escrow](./stayke-escrow.guide.md).

## Camino rápido

1. Vista ecosistema → diagrama 1.
2. Booking feliz → diagrama 2 / secuencia.
3. Cancelaciones → diagrama 3.
4. Disputa P2P-first (ventana 24 h → escalado → `resolve_dispute(outcome)`) → diagrama 4.
5. Onboarding: `init_identity` / `link_identity` → diagrama 6.

## Detalles

### Convenciones

| Símbolo | Significado |
|---------|-------------|
| `CPI` | Cross-Program Invocation (en labels de flowchart se omite `{}` por compatibilidad del parser) |
| `Lectura` | Lectura de estado sin mutar |
| Admin | Authority / admin de DisputeConfig |
| *permissionless* | Cualquier usuario puede llamar; el programa valida las condiciones |

### 1. Vista general del ecosistema

```mermaid
flowchart LR
    subgraph "stayke-config"
        GC[GlobalConfig<br/>fee_bps · minimum_deposit · free_ops<br/>usdc_mint · platform_vault<br/>core/escrow/disputes/treasury IDs<br/>authority]
    end

    subgraph "stayke-core"
        UP[UserProfile<br/>deposited · banned<br/>active_booking]
        RP[ReputationProfile<br/>client_cancellations<br/>host_cancellations]
        ID[Identity]
        LI[Listing<br/>total_reviews · rating]
    end

    subgraph "stayke-escrow"
        BK[Booking<br/>guest · host · property<br/>check_in · check_out<br/>total_price · status<br/>host_review · guest_review]
        EC[EscrowConfig<br/>authority · is_initialized]
        BD["BookingDays<br/>occupied_days: [u32;12]<br/>year"]
    end

    subgraph "stayke-disputes"
        DS[Dispute<br/>opened_by · state · outcome<br/>guest_evidence · host_evidence<br/>original_booking_status]
        DCFG["DisputeConfig<br/>admins (max 3)<br/>retribution_bps_*"]
    end

    subgraph "stayke-treasury"
        TV[(Treasury Vault)]
    end

    GC -.->|Lectura fee_bps, mint, vault, allowlist| BK
    GC -.->|Lectura| TV
    UP -.->|Lectura identity, deposited, banned| BK
    LI -.->|Lectura| BK
    DCFG -.->|Lectura admins| DS

    BK -->|CPI increment_completed_stays<br/>clear_active_booking<br/>increment_hosted_stays| UP
    BK -->|CPI increment_client_cancellations<br/>increment_host_cancellations| RP
    BK -->|CPI update_host_review<br/>update_client_review<br/>update_listing_review| LI

    DS -->|CPI cpi_update_booking_status<br/>congela Disputed / restaura original| BK
    DS -->|CPI cpi_resolve_dispute_transfer| BK
    DS -->|CPI cpi_penalize_transfer<br/>vía resolve_dispute| TV
    DS -->|CPI add_infraction · update_deposit<br/>vía resolve_dispute| UP
    DS -->|CPI increment_* / clear_active_booking<br/>vía close_dispute · ruta admin| UP
    TV -->|CPI update_deposit| UP
```

- `escalate_dispute` y `link_evidence` solo mutan la cuenta `Dispute`; `solve_dispute_before_admin` / `close_dispute` además mutan booking/Core vía CPI (`escalate` y `close` son permissionless).
- `clear_listing_booking` sigue existiendo en core pero **ya no tiene callers** (CPI sin uso).

### 2. Ciclo de vida Booking

```mermaid
stateDiagram-v2
    [*] --> Pending: create_booking / create_booking_cross_year

    Pending --> HostAccepted: host_accept_booking
    Pending --> Cancelled: host_reject_booking(_cross_year)
    Pending --> Cancelled: expire_booking(_cross_year) (24 h sin respuesta)

    HostAccepted --> Active: booking_starts (*permissionless*, check-in reached)
    HostAccepted --> Cancelled: guest_cancel_booking(_cross_year)
    HostAccepted --> Cancelled: host_cancel_booking(_cross_year)

    Active --> Completed: booking_completes (*permissionless*, check-out reached)
    Active --> Disputed: open_dispute CPI
    Completed --> Disputed: open_dispute CPI (sin ventana de tiempo)

    Completed --> Released: release_funds (*permissionless*, +24 h)

    Disputed --> DisputeResolved: resolve_dispute(outcome) vía CPI (escrow drenado)
    Disputed --> Active: restaura original_booking_status
    Disputed --> Completed: restaura original_booking_status

    note right of Released : fee → platform_vault<br/>host_amount → host<br/>completed_stays++, hosted_stays++<br/>vault cerrado (rent → caller)
    note right of Cancelled : guest cancel → split (60% / 75%) o full<br/>host cancel → full refund + slash 10%<br/>expire / reject → full refund al guest<br/>days liberados
    note right of Disputed : solo Active|Completed al abrir<br/>retiro P2P (≤ 24 h) o NoFaultFound<br/>restauran Active/Completed → release_funds
    
    Released --> [*]
    Cancelled --> [*]
    DisputeResolved --> [*]
```

`DisputeRejected` queda en el enum de `BookingStatus` pero **ninguna ruta lo produce hoy**: la resolución siempre termina en `DisputeResolved`.

### Secuencia — reserva feliz

```mermaid
sequenceDiagram
    participant G as Guest
    participant H as Host
    participant E as stayke-escrow
    participant C as stayke-core
    participant CF as stayke-config

    G->>E: create_booking(check_in, check_out)
    E->>C: {Lectura} guest profile: identity, banned, deposited
    E->>C: {Lectura} host profile: identity, banned, deposited
    E->>CF: {Lectura} GlobalConfig: minimum_deposit, usdc_mint
    Note over E: Gate: deposited >= minimum_deposit (guest y host, free_ops bypass)
    E->>E: USDC → escrow token account (PDA by booking)
    E->>E: Reserve days in BookingDays
    E-->>G: Pending

    H->>E: host_accept_booking
    E-->>H: HostAccepted

    Note over E: —check-in timestamp—

    Any->>E: booking_starts (*permissionless*)
    E->>C: {CPI} set_active_booking (guest)
    E-->>Any: Active

    Note over E: —check-out timestamp—

    Any->>E: booking_completes (*permissionless*)
    E-->>Any: Completed
    Note over E: updated_at = now → ventana de 24 h para release_funds

    Note over E: —ventana de 24 h (release_funds)—

    Any->>E: release_funds (*permissionless*)
    E->>E: fee = total_price × fee_bps / 10_000
    E->>H: host_amount = total_price − fee
    E->>CF: fee → platform_vault
    E->>C: {CPI} increment_completed_stays (guest)
    E->>C: {CPI} clear_active_booking (guest)
    E->>C: {CPI} increment_hosted_stays (host)
    E->>E: Close escrow token account → rent to caller
    E-->>Any: Released

    opt Reviews (dentro de la ventana post-estancia)
        G->>E: guest_review(score)
        E->>C: {CPI} update_host_review + update_listing_review
        H->>E: host_review(score)
        E->>C: {CPI} update_client_review
    end
```

### 3. Cancelaciones

```mermaid
sequenceDiagram
    participant G as Guest
    participant H as Host
    participant E as stayke-escrow
    participant C as stayke-core
    participant T as stayke-treasury

    Note over G,T: ─── Guest cancel ───
    alt Fuera de la ventana (> 72 h antes del check-in)
        G->>E: guest_cancel_booking(_cross_year)
        E->>G: full refund (100%)
    else Dentro de la ventana (≤ 72 h antes)
        G->>E: guest_cancel_booking(_cross_year)
        E->>G: 60% del total
        E->>H: 75% del remainder → host
        E->>E: 25% del remainder → platform_vault
    end
    E->>C: {CPI} increment_client_cancellations
    E->>E: Release BookingDays + close escrow
    E-->>G: Cancelled

    Note over G,T: ─── Host cancel ───
    alt Fuera de la ventana (> 72 h antes del check-in)
        H->>E: host_cancel_booking(_cross_year)
        E->>G: full refund (100%)
        Note over E: Sin slash al depósito
    else Dentro de la ventana (≤ 72 h antes)
        H->>E: host_cancel_booking(_cross_year)
        E->>G: full refund (100%)
        E->>T: {CPI} cpi_penalize_transfer (10% del depósito host)
        E->>C: {CPI} update_deposit (decrement host)
    end
    E->>C: {CPI} increment_host_cancellations
    E->>E: Release BookingDays + close escrow
    E-->>H: Cancelled
```

### 4. Disputa — P2P-first

Secuencia (detalle de validaciones en [disputes](./stayke-disputes.guide.md)):

```mermaid
sequenceDiagram
    participant OP as Opener (guest o host)
    participant CP as Contraparte (host o guest)
    participant Any as Cualquiera
    participant D as stayke-disputes
    participant E as stayke-escrow
    participant C as stayke-core
    participant T as stayke-treasury
    participant A as Admin (DisputeConfig.admins)

    OP->>D: open_dispute
    Note over D: Booking en Active o Completed · state = OpenP2P · ventana 24 h
    D->>E: {CPI} cpi_update_booking_status(Disputed)

    opt Retiro P2P (≤ 24 h desde opened_at, solo el opener)
        OP->>D: solve_dispute_before_admin
        D->>E: {CPI} cpi_update_booking_status(original_booking_status)
        Note over D,E: ResolvedByP2P → release_funds continúa normal
    end

    opt Escalado a admin (> 24 h, *permissionless*)
        Any->>D: escalate_dispute
        Note over D: state = Escalated
        OP->>D: link_evidence(hash_de_32_bytes)
        CP->>D: link_evidence(hash_de_32_bytes)
        Note over D: evidencia del guest o del host (opener o contraparte) · solo en Escalated
    end

    opt Resolución admin (solo en Escalated)
        A->>D: resolve_dispute(outcome)
        alt outcome con culpable (GuestFavored / HostFavored / MaliciousClaim)
            D->>C: {CPI} add_infraction(severity)
            D->>E: {CPI} cpi_resolve_dispute_transfer(slash_bps)
            Note over D,E: fee siempre · split víctima/culpable (vault cerrado → DisputeResolved)
            D->>T: {CPI} cpi_penalize_transfer(victim_amount)
            D->>C: {CPI} update_deposit(culpable, −slash)
        else NoFaultFound
            Note over D: Sin CPI de fondos · booking sigue Disputed
        end
        Note over D: state = ResolvedByAdmin
    end

    Any->>D: close_dispute (*permissionless*, solo si ResolvedByAdmin o ResolvedByP2P)
    alt booking DisputeResolved (ruta admin con escrow drenado)
        D->>C: {CPI} increment_completed_stays / clear_active_booking / increment_hosted_stays
    else booking Disputed (NoFaultFound)
        D->>E: {CPI} cpi_update_booking_status(original) → release_funds normal
    end
    Note over D: state = Closed · rent de la cuenta Dispute → wallet del opener
```

Máquina de estados de la cuenta `Dispute`:

```mermaid
stateDiagram-v2
    [*] --> OpenP2P: open_dispute (booking → Disputed)

    OpenP2P --> ResolvedByP2P: solve_dispute_before_admin (solo opener, ≤ 24 h)
    OpenP2P --> Escalated: escalate_dispute (*permissionless*, > 24 h)

    Escalated --> ResolvedByAdmin: resolve_dispute(outcome) (admin, en Escalated)

    ResolvedByP2P --> Closed: close_dispute (*permissionless*)
    ResolvedByAdmin --> Closed: close_dispute (*permissionless*)

    note right of OpenP2P : link_evidence solo en Escalated<br/>aplica a guest y host (opener o contraparte)
    note right of Escalated : evidencia → guest_evidence / host_evidence
```

- `penalize_user` ya **no existe**: la penalización (treasury + reputación + depósito) vive dentro de `resolve_dispute(outcome)`.
- `resolve_dispute` exige `dispute.state == Escalated` (`DisputeNotEscalated`) y admin en `config.admins` (`UnauthorizedAdmin`).
- `solve_dispute_before_admin` exige ser el opener (`UnauthorizedDisputeSolver`) y estar dentro de la ventana (`P2PWindowElapsed` en caso contrario).
- `link_evidence` acepta al **guest o al host del booking** (opener o contraparte), solo en `Escalated`; cualquier otro firmante → `UnauthorizedUser`. `EvidenceLinked` sigue definido pero sin enforce (la evidencia se sobrescribe).

### 5. Garantías (Treasury)

```mermaid
sequenceDiagram
    participant U as User
    participant T as stayke-treasury
    participant C as stayke-core
    participant CF as stayke-config

    U->>T: deposit_guarantee(amount)
    T->>CF: {Lectura} minimum_deposit, mint
    T->>T: USDC → treasury_vault
    T->>C: {CPI} update_deposit(+)
    U->>T: withdraw_guarantee(amount)
    T->>C: {CPI} update_deposit(-)
    T->>U: USDC ← vault
```

### 6. Onboarding identity (Core real)

```mermaid
flowchart TD
    U[Usuario] -->|initialize_user_profile| CORE[stayke-core]
    CORE --> UP[UserProfile + ReputationProfile]
    AD[Authority Core] -->|init_identity| ID[Identity PDA]
    AD -->|link_identity| LINK[UserProfile.identity = Some]
    LINK --> DEP[deposit_guarantee en treasury]
    DEP --> READY[Listo para create_booking si gates OK]
```

No hay `verify_identity` ni `set_host_status` en el programa actual.

## Cancellation policy — MVP constants

| Constant | Value | Descripción |
| --- | --- | --- |
| `CANCELLATION_WINDOW_HOURS` | `72` | Horas antes del check-in dentro de las cuales aplica el split |
| `CANCELLATION_REFUND_PERCENTAGE` | `60` | % del total que recibe el guest dentro de la ventana |
| `CANCELLATION_HOST_SHARE_PERCENTAGE` | `75` | % del remainder post-refund que recibe el host dentro de la ventana |
| `HOST_CANCELLATION_PENALTY_PERCENTAGE` | `10` | % del depósito del host que se quita en cancelación tardía |

> Los montos se calculan como remainder para que guest + host + Stayke sumen exactamente `total_price` sin pérdida por redondeo.

## Policy SoT vs On-chain gate

El gate `deposited >= minimum_deposit` en `create_booking` (y related) diverge de L1/L4 SoT. Callout completo: [escrow](./stayke-escrow.guide.md).

## Gaps

- `GlobalConfig` ya persiste los 4 program IDs (`core/escrow/disputes/treasury`) — allowlist `AllowedCaller` en `assert_cpi_authority`; ya no es un gap.
- `clear_listing_booking` (core) queda sin callers (CPI no usado por escrow/disputes).
- Disputas: sin ventana de apertura (un `Completed` puede disputarse sin límite) y admin único sin quórum (solo el authority del init, sin add/remove). ~~Evidencia solo del opener~~ → **resuelto**: `link_evidence` acepta a guest u host (opener o contraparte); `EvidenceLinked` sigue sin enforce (sobrescribe) → [disputes](./stayke-disputes.guide.md).
- Vault del escrow sin cerrar en `expire_booking` (ambas variantes) y `host_reject_booking_cross_year` (rent bloqueada) → [escrow](./stayke-escrow.guide.md).
- Yield no diagramado como live → [ADR-010](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-010-yield-deferred-stage-2.md).

## Checklist

- [x] Diagramas muestran `booking_starts` / `booking_completes` como permissionless
- [x] Diagrama de cancelaciones con split policy y host slash
- [x] `release_funds` como settlement permissionless post-24 h
- [x] Reviews separados: `guest_review` + `host_review` (no solo `review_completed`)
- [x] Diagrama 4 refleja el flujo P2P-first (ventana 24 h, escalado, close permissionless) y que `penalize_user` ya no existe
- [x] `resolve_dispute` integra escrow + treasury + reputación (la resolución admin ya no es "solo escrow")
- [x] Onboarding usa `init_identity` / `link_identity`
- [x] Cross-year variants documentados en las transiciones
- [ ] Sin footer de rama/archivos stale