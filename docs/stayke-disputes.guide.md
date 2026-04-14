# 📖 Guía del Contrato `stayke-disputes`

El contrato `stayke-disputes` es el árbitro del sistema. Su responsabilidad principal es escuchar y procesar casos en los que la estadía entre un huésped (client) y un anfitrión (host) no sale como se esperaba, resolviendo el conflicto financiero y penalizando reputacionalmente si es necesario.

---

## 🛠️ Funciones (Instrucciones)

A continuación, se describen las funciones principales expuestas por este contrato:

### 1. Configuración de Administrador
- **`initialize_config`**
  - **Propósito:** Configura los parámetros iniciales del contrato de disputas (como las wallets de sistema y autoridades permitidas para resolver conflictos).
- **`penalize_user`**
  - **Propósito:** Permite a un administrador del sistema emitir una penalización directa a un usuario (y CPI hacia su `ReputationProfile` en `stayke-core`). Recibe un nivel de severidad (`PenaltySeverity`).

### 2. Gestión de Disputas
- **`open_dispute`**
  - **Propósito:** Abre un caso de disputa formal. Puede ser llamado por el huésped o el anfitrión vinculado a una reserva específica (`booking`). Requiere un `DisputeReason` que categorice el problema.
  - **Efecto Secundario:** Cambia el estado de la reserva en `stayke-escrow` para evitar que los fondos de garantía sean liberados hasta que se dicte un veredicto.
- **`resolve_dispute`**
  - **Propósito:** Llamado por el Administrador/Votante para emitir una sentencia sobre la disputa abierta.
  - **Parámetros Clave:** 
    - `host_share_bps`: Porcentaje de los fondos bloqueados que le corresponde al anfitrión. El resto va al huésped.
    - `rejected`: Un boolean que indica si la disputa fue completamente rechazada (inválida).
  - **Efecto Secundario:** Realiza una llamada CPI a `stayke-escrow` (`cpi_resolve_dispute_transfer`) para mover el dinero y a `stayke-core` para penalizar reputacionalmente al infractor.
- **`close_dispute`**
  - **Propósito:** Cierra y archiva una cuenta de disputa una vez que todos los fondos han sido repartidos y las penalizaciones aplicadas, liberando el estado.

---

## 🔄 Flujo de Ejecución (Flow) Específico de Disputas

1. **Problema en la Estadía:** Durante o después del check-in, un usuario nota un incumplimiento y llama a `open_dispute` junto con la razón.
2. **Congelamiento:** El contrato de disputas notifica mediante CPI a `stayke-escrow` para que el dinero de la reserva quede en estado de "Disputado" (Frozen).
3. **Análisis:** El problema es examinado off-chain por el equipo de moderación o sistema descentralizado.
4. **Resolución:** El Admin llama a `resolve_dispute`, pasando los `basis points (bps)` y decidiendo la distribución de la plata. 
5. **Transferencias y Castigos:** Al resolver, el contrato se encarga de:
   - Pedirle a `stayke-escrow` que envíe los fondos a quien corresponda.
   - Pedirle a `stayke-treasury` penalizaciones extra de los depósitos según la gravedad.
   - Pedirle a `stayke-core` aumentar el nivel de infracciones de quien tuvo la culpa.
6. **Conclusión:** Se limpia el estado del sistema mediante `close_dispute`.
