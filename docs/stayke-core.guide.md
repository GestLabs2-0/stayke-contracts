# 📖 Guía del Contrato `stayke-core`

El contrato `stayke-core` es la base del ecosistema Stayke. Su propósito principal es gestionar el estado fundamental de los usuarios, incluyendo sus perfiles, sistemas de reputación, identidades (KYC/KYB base) y la gestión de propiedades (Listings). Funciona como la "fuente de verdad" para los demás contratos a la hora de verificar quién es un usuario y cuál es su historial.

---

## 🛠️ Funciones (Instrucciones)

A continuación, se describen las funciones principales expuestas por este contrato:

### 1. Inicialización y Configuración
- **`initialize_config`**
  - **Propósito:** Crea la cuenta global de configuración (`ConfigAcc`). Define la autoridad del sistema, que será quien pueda verificar identidades.
  - **Uso Común:** Llamado una sola vez por el administrador del protocolo en el despliegue.

### 2. Gestión de Usuarios e Identidad
- **`handler_initialize_user_profile`**
  - **Propósito:** Registra a un nuevo usuario en la plataforma. Crea tres PDAs vitales: `UserProfile`, `ReputationProfile`, y su cuenta de `Identity` ligada a un documento (ej. pasaporte o ID).
  - **Flujo:** Inicializa los datos en estado "no verificado".
- **`handler_verify_identity`**
  - **Propósito:** Verifica la cuenta de identidad de un usuario.
  - **Restricción:** Solo puede ser llamada por la `authority` definida en la configuración global. Cambia el estado del usuario (`is_verified = true`) y registra la fecha de verificación.

### 3. Gestión de Propiedades (Listings)
- **`initialize_listing`**
  - **Propósito:** Crea un nuevo listado (Listing) de una propiedad o alojamiento asociado a un usuario ya verificado.
- **`update_listing_price` / `update_listing_state`**
  - **Propósito:** Permiten al dueño del listado actualizar el precio o cambiar el estado (a través de un hash) de la propiedad respectiva.
- **`clear_listing_booking`**
  - **Propósito:** Libera un alojamiento marcándolo como `is_occupied = None`. Típicamente llamado cuando una estadía finaliza o se cancela.

### 4. Mutadores de Estado (Diseñados para CPI)
*Nota: Estas funciones mutan el estado base del usuario y están pensadas para ser llamadas por otros contratos (como Escrow o Disputes) de forma interconectada.*
- **`update_deposit`**
  - **Propósito:** Aumenta o disminuye el monto depositado (`deposited`) en el perfil de un usuario.
- **`set_host_status`**
  - **Propósito:** Cambia la bandera boolean `is_host` para designar si un usuario puede operar como anfitrión.
- **`clear_active_booking`**
  - **Propósito:** Remueve cualquier reserva activa en el perfil de un usuario, dejándolo libre para crear nuevas reservas.
- **`add_infraction`**
  - **Propósito:** Añade una infracción al `ReputationProfile` especificando la severidad (Baja, Media, Alta). Ideal para ser llamada desde el contrato de Disputas si un usuario es encontrado culpable de mal comportamiento.

---

## 🔄 Flujo de Ejecución (Flow) Específico de Core

El flujo de este contrato es lineal en cuanto a la integración al sistema, y se esquematiza así:

1. **Setup Inicial:** El Admin llama a `initialize_config` para parametrizar el núcleo.
2. **Onboarding de Usuario:** 
   - Un usuario llama a `initialize_user_profile` proporcionando su ID. 
   - Se crean sus perfiles de estado paralelos (`UserProfile` para datos, `ReputationProfile` para penalizaciones, e `Identity` para KYC).
3. **Verificación Manual (Off-chain + On-chain):** 
   - El sistema valida los datos de KYC de manera externa.
   - El Admin/Wallet Autorizada ejecuta `verify_identity` para activar al usuario.
4. **Interacción con el Ecosistema:**
   - Si el usuario quiere ser anfitrión, se registra y se agregan "Listings".
   - A lo largo del tiempo, contratos de terceros (como Escrow y Disputas) llamarán a los mutadores (como `update_deposit` o `add_infraction`) vía CPI para mantener actualizado el saldo garantizado (`deposit`) o el ranking de conducta moral en el protocolo.
