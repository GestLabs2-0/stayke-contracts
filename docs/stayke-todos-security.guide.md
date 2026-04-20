# 📋 Sistema Stayke: Lista de TODOs y Análisis de Seguridad

Este documento recopila todos los comentarios `TODO` encontrados a lo largo de los contratos inteligentes (Anchor/Rust) de Stayke y provee un análisis sobre las brechas de seguridad y mejoras pendientes en cada caso.

---

## 🔒 Brechas de Seguridad y Autorización Críticas (CPI)

Las siguientes notas representan vulnerabilidades potenciales donde la lógica de los contratos no está completamente sellada frente a interacciones maliciosas.

### 1. Falta de validación de llamadas cruzadas (CPI) en `stayke-core`
- **Ubicación:** `stayke-core/src/instructions/user_profile_mutators.rs:5`
  > _"TODO: enforce security. We don't allow modifications from other contracts unless we secure them beforehand"_
- **Ubicación:** `stayke-core/src/instructions/listing_mutator.rs:41`
  > _"TODO: enforce security. We don't allow modifications from other contracts unless we secure them beforehand"_ (y línea 45 relacionada con los seeds).
- **Riesgo:** Actualmente las funciones mutadoras (`update_deposit`, `clear_listing_booking`, etc.) aceptan mutar el estado si la instrucción es firmada, pero no validan firmemente que la firma de autoridad provenga estrictamente de un PDA autorizado (ej. el contrato Escrow o Disputes). Un atacante podría firmar con su wallet y falsificar su saldo u ocupación.
- **Solución:** Requerir que la `authority` que firme estos mutadores sea la dirección aprobada e inmutable definida en el nuevo `GlobalConfig`.

### 2. Validación de Cuentas de Tokens Débil en `stayke-disputes`
- **Ubicación:** `stayke-disputes/src/instructions/manage_disputes.rs:150`
  > _"TODO: add validations for token accounts. Platform and usdc_mint need to be equal to the other config files"_
- **Riesgo:** Si un contrato no verifica estrictamente el `mint` o las cuentas de tesorería (`platform_vault`), un atacante durante una resolución de disputa podría insertar una cuenta falsa y desviar fondos hacia otro lado en lugar del vault de la plataforma.
- **Solución:** Consumir la cuenta `GlobalConfig` en esta instrucción y hacer un constraint duro: `constraint = platform.key() == global_config.platform_vault`.

### 3. Riesgo de Centralización / Uso de Signers Inadecuados en `stayke-disputes`
- **Ubicación:** `stayke-disputes/src/instructions/manage_disputes.rs:253`
  > _"TODO: instead of the admin users, we must only use the account PDA as the signer, but for simplicity we can just use the admin signer for now."_
- **Riesgo:** Usar llaves privadas ("Admin Users") para firmar operaciones internas en lugar del PDA rompe la descentralización y presenta un punto de falla único (single point of failure) si el admin pierde su llave privada. Los CPIs deberían ser firmados autónomamente por semillas (PDA seeds).

---

## ⚙️ Tareas de Desarrollo y Lógica Pendiente

### `stayke-config`
- **Ubicación:** `stayke-config/src/state.rs:3`
  > _"TODO: add stayke contracts to Global Config"_
  - **Acción:** Relacionado directamente con la seguridad de la Sección 1. Hay que añadir los `Pubkeys` oficiales de los demás programas en `GlobalConfig` para que actúe como "Single Source of Truth".
- **Ubicación:** `stayke-config/src/lib.rs:26`
  > _"TODO: create instruction to withdraw fees from vault"_
  - **Acción:** El sistema recauda dinero, pero aún no tiene una función lógica para que el administrador retire la rentabilidad desde el `platform_vault`.

### `stayke-escrow`
- **Ubicación:** `stayke-escrow/src/instructions/booking.rs:709`
  > _"TODO: should I implement somekind of conditional if the host is banned. What happens to the money if the host is banned after the stay is completed but before the booking is closed?"_
  - **Acción:** Caso borde lógico. Definir el flujo de rescate de fondos en el caso muy específico de que al anfitrión se le bloquee la cuenta (ban) en medio de la finalización de una reserva.

### `stayke-core`
- **Ubicación:** `stayke-core/src/state/users.rs:27`
  > _"TODO: is this field required?"_
  - **Acción:** Revisar la estructura del estado de usuarios para eliminar bytes innecesarios en la PDA y ahorrar renta.

### `stayke-treasury` (Nuevos features de DeFi)
- **Ubicación:** `stayke-treasury/src/instructions/lending.rs` (Múltiples TODOs, líneas 8, 15, 25, 31, 41, 47)
  > _"TODO: In the future this instruction will allow users to lend their USDC..."_
  > _"TODO: Liquid staking placeholder..."_
  > _"TODO: Add lending/staking protocol accounts"_
  - **Acción:** El contrato tiene los stubs (cascarones) para integrar `staking` y `lending` usando protocolos composables de Solana (ej. Kamino o Marginfi). Queda pendiente agregar la lógica CPI correspondiente.
