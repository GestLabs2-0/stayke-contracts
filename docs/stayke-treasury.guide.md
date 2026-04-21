# 📖 Guía del Contrato `stayke-treasury`

El contrato `stayke-treasury` es la tesorería principal del protocolo. Gestiona los depósitos colateralizados de los usuarios (sus "garantías") requeridos para participar en la plataforma, custodia los ingresos operacionales, e incluye la lógica base (a futuro) para prestar fondos a protocolos de DeFi y hacer "staking" en favor de la rentabilidad del protocolo y sus usuarios.

---

## 🛠️ Funciones (Instrucciones)

A continuación, se describen las funciones principales expuestas por este contrato:

### 1. Configuración de Tesorería
- **`initialize_treasury`**
  - **Propósito:** Configura y activa el contrato. Establece el umbral de `minimum_deposit` que los usuarios deberán tener bloqueado para poder reservar o listar propiedades.

### 2. Gestión de Garantías de Usuarios
- **`deposit_guarantee`**
  - **Propósito:** Permite a un huésped o anfitrión enviar USDC (o el token configurado) como fondo de garantía al tesoro de la plataforma.
  - **Efecto Secundario:** Mediante CPI, actualiza la propiedad `deposited` en el `UserProfile` del usuario en el contrato `stayke-core` para reflejar el nuevo balance.
- **`withdraw_guarantee`**
  - **Propósito:** Le permite a un usuario retirar sus fondos colateralizados de vuelta hacia su wallet personal. Verifica que no haya reservas en curso, que no tenga penalidades graves y resta la cantidad correspondiente mediante un CPI en el `stayke-core`.

### 3. Lending y Staking (Placeholders DeFi)
*Nota: Estas funciones preparan a Stayke para operar como un protocolo puente y sacar rendimiento pasivo.*
- **`lend` y `withdraw_from_lending`**
  - **Propósito:** Tomar parte del fondo inactivo del tesoro y meterlo en plataformas de préstamo (lending). Luego sacarlo para reponer liquidez si es solicitado.
- **`stake`**
  - **Propósito:** Enviar los activos a protocolos externos de Staking Líquido. 

### 4. Extraer Fondos por Penalidad (CPI Flow)
- **`cpi_penalize_transfer`**
  - **Propósito:** Esta función sirve de "hacha" para las penalizaciones. Únicamente puede ser activada por una llamada originada desde el contrato `stayke-disputes`.
  - **Flujo:** Si un host o cliente comete una falla que merita un castigo económico dictado en disputas, se llama esta función transfiriendo dinero deducido del depósito del usuario (`deposited`) directamente al bolsillo de la plataforma (Vault Fee Principal). 

---

## 🔄 Flujo de Ejecución (Flow) Específico de Treasury

1. **Top-Up:** Antes de usar la app, el Usuario llama a `deposit_guarantee`. Sus tokens viajan a un Vault bajo custodia directa del programa `stayke-treasury`.
2. **Registro Contable:** El contrato `treasury` llama a `stayke-core` para anotarle `X` monto a favor en su perfil.
3. **Periodo de Retención:** 
   - El dinero permanece estático y sirve como colchón de riesgo (por daños, impagos, etc.). 
   - *(En el futuro, estos fondos ociosos serán canalizados paralelamente usando `lend` o `stake` para ganar rendimiento).*
4. **Castigo:** Si hay disputas y el participante es sancionado, `stayke-disputes` obliga al Treasury a extraer el valor de este fondo de compensación llamando a `cpi_penalize_transfer`.
5. **Egreso Seguro:** Un usuario libre de reservas manda a ejecutar `withdraw_guarantee`. Se le reintegran los tokens congelados directamente a su cuenta.
