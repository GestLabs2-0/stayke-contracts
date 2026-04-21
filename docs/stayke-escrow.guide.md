# 📖 Guía del Contrato `stayke-escrow`

El contrato `stayke-escrow` es el motor transaccional del flujo de reservas de Stayke. Asegura los pagos entre los huéspedes y anfitriones, bloqueando ("escrowing") el dinero durante la estadía para garantizar la confianza de ambas partes.

---

## 🛠️ Funciones (Instrucciones)

A continuación, se describen las funciones principales expuestas por este contrato:

### 1. Inicialización
- **`initialize_escrow`**
  - **Propósito:** Fija los parámetros base de la bóveda de escrow. Recibe comisiones (`fee_bps`) y otras configuraciones.

### 2. Ciclo de Vida de la Reserva (Booking Lifecycle)
Todo el flujo de una renta vacacional ocurre en los siguientes pasos:
- **`create_booking`**
  - **Propósito:** El huésped crea una solicitud de reserva especificando un periodo (`check_in` y `check_out`). Se reserva el calendario temporalmente, pero AÚN NO se mueven los fondos.
- **`host_accept_booking` / `host_reject_booking`**
  - **Propósito:** El anfitrión revisa la solicitud de reserva y decide aceptarla o rechazarla.
- **`client_accept_reserve` / `client_reject_reserve`**
  - **Propósito:** Una vez que el anfitrión aceptó, el cliente confirma la reserva llamando a `client_accept_reserve` (lo cual transfiere los fondos de USDC a la bóveda de Escrow asegurada) o puede cancelarla preventivamente con `client_reject_reserve`.
- **`review_completed`**
  - **Propósito:** Una vez finalizada la estadía, el huésped deja una calificación (score) de la experiencia. Modifica el estado a `ReviewCompleted` y actualiza la reputación del anfitrión de manera automática vía CPI interno.
- **`complete_stay`**
  - **Propósito:** Finaliza el proceso. Toma la reserva ya revisada, libera el dinero depositado en Escrow pagándole el total correspondiente al anfitrión (omitiendo la comisión que va a la bóveda de `Platform`) y cierra definitivamente las cuentas transaccionales de la reserva.

### 3. Integración CPI (Para Disputas)
- **`cpi_update_booking_status`**
  - **Propósito:** El contrato de Disputas llama a esta función para cambiar el estatus de la reserva (ej. a 'En Disputa') y evitar que `complete_stay` y transferencias normales se efectúen.
- **`cpi_resolve_dispute_transfer`**
  - **Propósito:** Cuando una disputa concluye en `stayke-disputes`, aquel contrato llama a esta función para dividir el botín según los porcentajes definidos y repartir los remanentes.

---

## 🔄 Flujo de Ejecución (Flow) Específico de Escrow

1. **Intención de Reserva:** El Cliente llama a `create_booking`. Se generan los registros de días, pero la plata sigue con el cliente.
2. **Confirmación del Host:** El Host revisa y aprueba con `host_accept_booking`.
3. **Bloqueo Monasterial (Pago):** El Cliente entonces ejecuta `client_accept_reserve`. Aquí es donde pasa la magia de seguridad: los fondos en USDC se mueven a una bóveda temporal única generada para esa reserva (Escrow Token Account).
4. **Periodo de Reserva:** El dinero permanece "escroweado" e intocable durante toda la estadía.
5. **Finalización Feliz:**
   - La estadía termina y el cliente llama a `review_completed(score)`.
   - Luego, se ejecuta `complete_stay` para liquidar las cuentas: el vault extrae su % de comisión y el Host recibe su pago directamente a su bóveda de USDC. Las cuentas temporales se destruyen limpiando la memoria del protocolo.
6. **Finalización Conflictiva (CPI Flow):** 
   - Si se levanta una disputa antes de finalizar, el estado de `Booking` se paraliza. Luego, Escrow acata ciegamente a `cpi_resolve_dispute_transfer` llamado desde `stayke-disputes` para redirigir la plata.
