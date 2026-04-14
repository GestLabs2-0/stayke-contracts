# 🏰 Arquitectura Global y Flujo de Interconexiones de Stayke

A pesar de existir de manera independiente, los 4 contratos inteligentes de Stayke (`stayke-core`, `stayke-disputes`, `stayke-escrow` y `stayke-treasury`) forman un ecosistema sumamente interconectado y modular donde todos confían en uno: la información es validada principalmente por el **Core**.

En este diagrama conceptual se detallan las principales responsabilidades de cada componente y cómo se comunican entre sí.

---

## 🔄 Rol de Cada Componente en el Ecosistema

1. **🌍 Core (`stayke-core`)**
   - **Es el cerebro y la fuente de la verdad para el estado de los usuarios.**
   - Mantiene perfiles, listas de propiedades (listings) y registros de reputación KYC.
2. **💰 Treasury (`stayke-treasury`)**
   - **Actúa como la base monetaria general (Garantías/Colaterales).**
   - Maneja el dinero de alto nivel y el fondo de riesgo aportado por los usuarios para siquiera abrir la app.
3. **🏦 Escrow (`stayke-escrow`)**
   - **Opera como el intermediario transaccional de "corto plazo".**
   - Solamente resguarda los pagos relacionados al flujo efímero de una reserva (`Booking`).
4. **⚖️ Disputes (`stayke-disputes`)**
   - **Actúa como el Tribunal Supremo Judicial.**
   - Congela movimientos de dinero, impone veredictos alterando pagos directos, golpea la reputación y absorbe partes de depósitos.

---

## 🔗 Matriz de Interacciones y Flujos de Lógica Transversal

El ecosistema Stayke maneja un diseño donde los procesos dependen de "Cross-Program Invocations" (CPI) para su completitud. El flujo general integrado se concibe de la siguiente manera:

### Flujo 1: Registro Onboarding Completo
- **A -> Usuario:** Llama e inicializa su Identidad y Perfil en `stayke-core`.
- **B -> Usuario:** Ejecuta un depósito en `stayke-treasury` aportando su capital de garantía mínimo. 
- **C -> Core:** Internamente gracias al CPI emitido por `treasury`, el registro del core del usuario eleva su límite de `deposited`.

### Flujo 2: Creación del Viaje Feliz (Reserva Perfecta)
- **A -> Core:** Ambas partes demuestran ser actrices válidos (Están verificados, no baneados, cuentas con buen standing).
- **B -> Escrow:** Se abre la puerta en `stayke-escrow` para aceptar los fondos en el vault temporal por los días solicitados por el listado alojado en el `Core`.
- **C -> Escrow:** Cuando los días acaban, y la estadía se marca como "Completada", el Host recibe los fondos desde la bóveda de escrow. Nadie es alertado; todo funciona en paz.
- **D -> Core:** En la finalización, la propiedad (`is_occupied`) se desmarca por CPI. 

### Flujo 3: El Flujo Crítico de Conflictos (Disputa en la Milla Extra)
*Este es el momento de máxima interconexión de todo el protocolo Stayke:*

1. **La Ruptura (`stayke-disputes`):** Al generarse un problema, la supuesta víctima abre formalmente un caso.
2. **Aviso de Alto Nivel (`stayke-disputes` ➔ `stayke-escrow`):** Inmediatamente, la disputa pide mediante CPI paralizar el estatus del Booking (`cpi_update_booking_status`), amarrando las manos y paralizando el escrow.
3. **Sentencia y Dinero (`stayke-disputes` ➔ `stayke-escrow`):** Al dictaminar un fallo con `resolve_dispute`, el juez instruye vía CPI (`cpi_resolve_dispute_transfer`) cómo repartir el botín que estaba secuestrado en Escrow.
4. **Deducciones de Riesgo (`stayke-disputes` ➔ `stayke-treasury`):** Si hay una multa o es necesario, la disputa entra al tesoro en sí y solicita transferir plata desde la "Garantía Base" llamando `cpi_penalize_transfer`. 
5. **Dedo Acusador (`stayke-disputes` ➔ `stayke-core`):** Finalmente el jurado mancha permanentemente la métrica del infractor mutando su cantidad de "Infractions" en el perfil centralizado con todo el poder moral y sistemático.

---
> [!NOTE] 
> Todas las flechas de ejecución asumen un protocolo subyacente donde las firmas entre PDAs (Program Derived Addresses) actúan con base en configuraciones seguras para salvaguardar el estado de llamadas maliciosas externas **(la "Global Config", pendiente de integrar vía una capa superior como discutimos antes).**
