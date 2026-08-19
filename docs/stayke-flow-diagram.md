# Diagramas de flujo — Stayke Contracts (as implemented)

> **As implemented** — documenta el código on-chain actual (`programs/**`), no la política de producto.
> **SoT (norma):** [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md). Si hay conflicto, manda la SoT; aquí solo se describen gaps explícitos.

Mermaid alineado a `lib.rs` / CPI actuales. Referencia de cuentas: [stayke-global.guide.md](./stayke-global.guide.md).

## Camino rápido

1. Vista ecosistema → diagrama 1.
2. Booking feliz → diagrama 2 / secuencia.
3. Cancelaciones → diagrama 3.
4. Disputa: `resolve` ≠ `penalize` → diagrama 4.
5. Onboarding: `init_identity` / `link_identity` → diagrama 6.

## Detalles

### Convenciones

| Símbolo | Significado |
|---------|-------------|
| `{CPI}` | Cross-Program Invocation |
| `{Lectura}` | Lectura de estado sin mutar |
| Admin | Authority / admin de DisputeConfig |
| *permissionless* | Cualquier usuario puede llamar; el programa valida las condiciones |

### 1. Vista general del ecosistema

```mermaid
flowchart LR
    subgraph "stayke-config"
        GC[GlobalConfig<br/>fee_bps · minimum_deposit<br/>usdc_mint · platform_vault<br/>authority]
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
        BD[BookingDays<br/>occupied_days: [u32;12]<br/>year]
    end

    subgraph "stayke-disputes"
        DS[Dispute]
    end

    subgraph "stayke-treasury"
        TV[(Treasury Vault)]
    end

    GC -.->|{Lectura} fee_bps, mint, vault| BK
    GC -.->|{Lectura}| TV
    UP -.->|{Lectura} identity, deposited, banned| BK
    LI -.->|{Lectura}| BK

    BK -->|{CPI} increment_completed_stays<br/>clear_active_booking<br/>increment_hosted_stays| UP
    BK -->|{CPI} increment_client_cancellations<br/>increment_host_cancellations| RP
    BK -->|{CPI} update_host_review<br/>update_client_review<br/>update_listing_review| LI

    DS -->|{CPI} cpi_update_booking_status| BK
    DS -->|{CPI} cpi_resolve_dispute_transfer| BK
    DS -->|{CPI} cpi_penalize_transfer<br/>vía penalize_user| TV
    DS -->|{CPI} add_infraction · update_deposit<br/>vía penalize_user| UP
    DS -->|{CPI} clear_* vía close_dispute| UP
    DS -->|{CPI} clear_listing_booking| LI

    TV -->|{CPI} update_deposit| UP
```

### 2. Ciclo de vida Booking

```mermaid
stateDiagram-v2
    [*] --> Pending: create_booking / create_booking_cross_year

    Pending --> HostAccepted: host_accept_booking
    Pending --> Cancelled: host_reject_booking
    Pending --> Cancelled: expire_booking (24 h sin respuesta)

    HostAccepted --> Active: booking_starts (permissionless, check-in reached)
    HostAccepted --> Cancelled: guest_cancel_booking
    HostAccepted --> Cancelled: host_cancel_booking

    Active --> Completed: booking_completes (permissionless, check-out reached)
    Active --> Disputed: open_dispute CPI

    Completed --> Released: release_funds (permissionless, 24 h después)

    Disputed --> DisputeResolved: resolve_dispute(rejected=false)
    Disputed --> DisputeRejected: resolve_dispute(rejected=true)

    note right of Released : fee → platform_vault<br/>host_amount → host<br/>completed_stays++, hosted_stays++<br/>escrow cerrado
    note right of Cancelled : guest cancel → refund split o full<br/>host cancel → full refund + posible slash<br/>expire → full refund al guest<br/>days released, booking cerrado

    Released --> [*]
    Cancelled --> [*]
    DisputeResolved --> [*]
    DisputeRejected --> [*]
```

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
    Note over E: Gate: deposited >= minimum_deposit (guest y host)
    E->>E: USDC → escrow token account (PDA by booking)
    E->>E: Reserve days in BookingDays
    E-->>G: Pending

    H->>E: host_accept_booking
    E-->>H: HostAccepted

    Note over E: —等待 check-in timestamp—

    Any->>E: booking_starts (*permissionless*)
    E->>C: {CPI} set_active_booking (guest)
    E-->>Any: Active

    Note over E: —等待 check-out timestamp—

    Any->>E: booking_completes (*permissionless*)
    E-->>Any: Completed
    Note over E: updated_at = now → 24 h dispute window starts

    Note over E: —24 h dispute window—

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
    alt Fuera de la ventana (72 h antes del check-in)
        G->>E: guest_cancel_booking
        E->>G: full refund (100%)
    else Dentro de la ventana (≤72 h antes)
        G->>E: guest_cancel_booking
        E->>G: 60% del total
        E->>H: 75% del remainder → host
        E->>E: 25% del remainder → platform_vault
    end
    E->>C: {CPI} increment_client_cancellations
    E->>E: Release BookingDays + close escrow
    E-->>G: Cancelled

    Note over G,T: ─── Host cancel ───
    alt Fuera de la ventana (72 h antes del check-in)
        H->>E: host_cancel_booking
        E->>G: full refund (100%)
        Note over E: Sin slash al deposito
    else Dentro de la ventana (≤72 h antes)
        H->>E: host_cancel_booking
        E->>G: full refund (100%)
        E->>T: {CPI} cpi_penalize_transfer (10% del deposito host)
        E->>C: {CPI} update_deposit (decrement host)
    end
    E->>C: {CPI} increment_host_cancellations
    E->>E: Release BookingDays + close escrow
    E-->>H: Cancelled
```

### 4. Disputa — resolve ≠ penalize

```mermaid
sequenceDiagram
    participant U as Initiator
    participant D as stayke-disputes
    participant E as stayke-escrow
    participant T as stayke-treasury
    participant C as stayke-core
    participant A as Admin

    U->>D: open_dispute(reason)
    D->>E: {CPI} cpi_update_booking_status(Disputed)
    Note over D: Booking debe estar en Active o Completed

    A->>D: resolve_dispute(host_share_bps, rejected)
    D->>E: {CPI} cpi_resolve_dispute_transfer
    Note over D,E: Solo escrow del booking — sin Treasury ni reputación

    opt Penalización de bond / reputación
        A->>D: penalize_user(severity)
        D->>T: {CPI} cpi_penalize_transfer
        D->>C: {CPI} update_deposit(-)
        D->>C: {CPI} add_infraction
    end

    A->>D: close_dispute
    D->>C: {CPI} clear_active_booking / clear_listing_booking
```

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
| `HOST_CANCELLATION_PENALTY_PERCENTAGE` | `10` | % del deposito del host que se quita en cancelación tardía |

> Los montos se calculan como remainder para que guest + host + Stayke sumen exactamente `total_price` sin pérdida por redondeo.

## Policy SoT vs On-chain gate

El gate `deposited >= minimum_deposit` en `create_booking` (y related) diverge de L1/L4 SoT. Callout completo: [escrow](./stayke-escrow.guide.md).

## Gaps

- `GlobalConfig` sin program IDs (diagrama 1).
- Yield no diagramado como live → [ADR-010](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-010-yield-deferred-stage-2.md).
- Validaciones token / PDA signer en disputes → [security](./stayke-todos-security.guide.md).

## Checklist

- [x] Diagramas muestran `booking_starts` / `booking_completes` como permissionless
- [x] Diagrama de cancelaciones con split policy y host slash
- [x] `release_funds` como settlement permissionless post-24 h
- [x] Reviews separados: `guest_review` + `host_review` (no solo `review_completed`)
- [x] `resolve_dispute` no implica treasury
- [x] Onboarding usa `init_identity` / `link_identity`
- [x] Cross-year variants documentados en las transiciones
- [ ] Sin footer de rama/archivos stale
