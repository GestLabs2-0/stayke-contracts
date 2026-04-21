# 📖 Guía del Contrato `stayke-config`

El contrato `stayke-config` actúa como la configuración global ("Global Config") y bóveda central del sistema. Su propósito es ser la fuente única de verdad (Single Source of Truth) para el ecosistema completo, eliminando la necesidad de variables fijas y consolidando el control de acceso, los tesoros y las comisiones en un solo lugar.

## 🛠️ Funciones (Instrucciones)

A continuación, se describen las funciones principales expuestas por este contrato:

### 1. Inicialización Global
- **`initialize_config`**
  - **Propósito:** Configura los parámetros maestros del sistema. Acepta:
    - `minimum_deposit`: El monto de depósito mínimo requerido a un usuario para operar en Stayke.
    - `fee_bps`: El porcentaje de comisión general (en basis points) que cobra la plataforma en cada estadía exitosa.
  - **Efectos y PDAs:** Al ser llamado también inicializa un estado llamado `GlobalConfig` el cual almacena la identidad de la autoridad (`authority`), la moneda autorizada del protocolo (`usdc_mint`), y crea la `Platform Vault` (junto a su autoridad derivada), que es la cuenta en donde se van a recibir todas las utilidades extraídas del Escrow por comisiones.

### 2. Futuras Funciones (Pendientes en TODO)
- **Extracción de Comisiones (Withdraw Fees)**:
  - Existe un "TODO" que indica la necesidad de crear una instrucción que le permita a la `authority` retirar el dinero recolectado en la `platform_vault`.

---

## 🔄 Flujo de Ejecución (Rol en el sistema)

Este contrato es pasivo a nivel de interacción de usuario, pero activamente vital para el sistema:

1. **Fase de Despliegue:** El admin del sistema manda a inicializar `stayke-config`, creando así la única fuente de estado de la cual derivarán las reglas.
2. **Autorización y CPIs Seguros:**  Como lo anotaste en el código, el propósito clave de este contrato es que cada vez que `stayke-core`, `stayke-escrow` o cualquier otro contrato haga un CPI, no se tengan que hardcodear direcciones. En lugar de eso, referencian los public keys registrados on-chain en el `GlobalConfig`.
3. **Distribución Escrow:** Cuando el `stayke-escrow` cierra una reserva (`complete_stay`), ahora envía la tarifa final pre-establecida (`fee_bps`) a la token account única de la plataforma guardada y administrada por `stayke-config`.
