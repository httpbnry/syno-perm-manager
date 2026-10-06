# Syno Perm Manager

Gestor visual de permisos ACL, usuarios y grupos para Synology NAS via SSH. Aplica permisos a multiples carpetas simultaneamente, analiza accesos por usuario, y administra cuentas — todo desde una app desktop en vez de ir carpeta a carpeta en DSM.

## Por que SSH y no la API REST de Synology

Synology ofrece una API REST (DSM API) para gestionar el NAS remotamente. Sin embargo, **no sirve para gestionar ACLs POSIX**:

| | API REST de Synology | SSH + synoacltool |
|---|---|---|
| **Permisos disponibles** | Solo SMB basico (RW/RO/Deny) | 14 permisos granulares (r, w, x, p, d, D, a, A, R, W, c, C, o, s) |
| **Flags de herencia** | No soportados | 4 flags (f, d, i, n) — control total de herencia |
| **Allow/Deny por entrada** | Limitado | Cada entrada puede ser allow o deny |
| **Gestion de usuarios** | API basica de usuarios | synouser completo (crear, borrar, renombrar, password, expirar) |
| **Gestion de grupos** | API basica de grupos | synogroup completo (crear, borrar, miembros, renombrar) |
| **Snapshots de ACL** | No | synoacltool -get + restauracion completa |
| **Batch masivo** | Una carpeta por llamada | N carpetas en un solo comando |

**En resumen:** la API REST es como usar la puerta principal de una casa — solo llegas al salon. SSH es la llave maestra que abre todas las habitaciones, incluido el sotano donde estan los permisos reales del sistema de archivos.

## Caracteristicas

### Comparar y copiar permisos entre usuarios
- Nueva sección **Comparar permisos**, accesible desde el menú, inicio, ficha de usuario y al finalizar un alta.
- **1. Grupos:** tabla con pertenencia de origen y destino, resultado previsto y orden de aplicación (primero grupos, después carpetas).
- **2. Carpetas específicas:** por cada carpeta, permisos manuales del usuario (`manual`), heredados de usuario (`inherited`), obtenidos por grupo (`group`) y de `everyone`, con explicación de cada letra `rwx...`, tipo `allow/deny`, flags y nivel de herencia.
- **3. Cambios:** lista filtrable de altas/bajas exactas antes de aplicar.
- **Añadir**: incorpora grupos y entradas que faltan, conservando las asignaciones del destino.
- **Actualizar**: sustituye las ACL directas con el mismo tipo `allow/deny` y flags de herencia que las del origen; conserva las demás y añade grupos.
- **Reemplazar**: iguala las pertenencias a grupos y las ACL directas del destino dentro de las carpetas elegidas; retira las adicionales.
- Selección de carpetas compartidas, botón para seleccionarlas todas, rutas específicas de cualquier volumen y análisis recursivo opcional (hasta 5000 carpetas, máximo 100 raíces).
- Vista previa obligatoria, planes de un solo uso que caducan a los 15 minutos, relectura completa antes de modificar y cancelación del análisis al cambiar de sección.
- Registro del estado anterior en `user_sync_backup`, exportable como JSON desde Historial de cambios; resultados en `user_sync`.

**Alcance:** copia usuarios locales existentes. Para un usuario nuevo, créalo primero con el asistente y pulsa **Copiar permisos de otro usuario**. La copia modifica exclusivamente las ACE directas del destino; las heredadas deben copiarse en el padre que las origina. Los grupos afectan a todo el NAS, aunque selecciones pocas carpetas. No se copian propietarios, ACL específicas de archivos, permisos de aplicaciones DSM ni configuración de cuentas. No es un cálculo de acceso efectivo de DSM.

**Aplicación parcial:** SSH no ofrece una transacción que abarque todos los cambios. Se aplica en orden (grupos, luego carpetas hijas antes que padres), se detiene en el primer error y muestra cuántas operaciones terminaron; vuelve a comparar para comprobar el resultado o continuar. El respaldo de comparación permite recuperación manual, no tiene deshacer automático. Evita editar simultáneamente esos permisos desde DSM mientras se aplica el plan.

### Interfaz y rendimiento
- Nueva navegación con iconos, cabecera de conexión, panel de inicio y diseño común de tarjetas, tablas y formularios.
- Temas claro/oscuro persistentes, navegación por teclado e indicadores de foco.
- Lecturas SSH deduplicadas y cacheadas (30 s en interfaz, 5 min en ACL), sin reencolar al cambiar rápido de sección; los resultados obsoletos se descartan.
- Comparaciones paginadas, filtro de carpetas y rutas canónicas deduplicadas.
- Resolución de la ruta real de cada carpeta compartida mediante `synoshare --get`, sin asumir `/volume1`.

### Gestion de permisos
- **Multi-seleccion en lote**: selecciona N carpetas y aplica los mismos permisos a todas a la vez
- **Editor visual de ACL**: arbol de permisos con checkboxes padre/hijo y estado indeterminado
- **14 permisos POSIX de Synology**: Read, Write, Execute, Append, Delete, Delete-child, read/write Attribute, read/write xattr, read/write ACL, Ownership, Sync
- **4 flags de herencia**: File-inherit, Dir-inherit, Inherit-only, No-propagate
- **Plantillas rapidas**: Lectura, Escritura, Personalizado — un clic para configurar
- **Preview / dry-run**: ve los cambios antes de aplicarlos, con diff lado a lado
- **Recursividad opcional**: aplica permisos a subcarpetas con un checkbox
- **Modo aditivo**: anade permisos sin sobrescribir los existentes de otros usuarios

### Analisis de permisos
- **Matriz de permisos**: grid visual usuarios x carpetas con colores (rojo/naranja/verde)
- **Coloreado por usuario**: selecciona un usuario/grupo y ve su acceso en el arbol
- **Deteccion de overrides**: marca carpetas donde un hijo tiene permisos distintos al padre
- **Filtro de conflictos**: muestra solo las carpetas con overrides
- **Exportar CSV**: descarga la matriz completa para Excel/Informes
- **Cache SQLite**: las ACLs se cachean para no repetir peticiones SSH

### Gestion de usuarios y grupos
- **Wizard de alta**: crea usuario + asigna grupos + aplica permisos en un solo flujo de 3 pasos
- **Auto-generacion**: escribe el nombre completo y se genera usuario, password aleatoria y descripcion
- **CRUD completo**: crear, borrar, renombrar, cambiar password
- **Gestion de miembros**: anadir/quitar usuarios de grupos visualmente
- **Detalle de usuario**: UID, GID, email, home, shell, grupos, expiracion
- **Detalle de grupo**: GID, tipo, descripcion, miembros

### Auditoria y seguridad
- **Historial con filtros**: búsqueda, conexión, acción, resultado y rango de fechas, con paginación en servidor.
- **Detalle por operación**: contenido completo, snapshots asociados y exportación individual.
- **Snapshots ACL**: previsualización antes de restaurar, confirmación obligatoria y restauración limitada a la conexión propietaria.
- **Respaldos de comparación**: exportación JSON, reintento de comparación y recuperación manual documentada.
- **Exportación de informes**: CSV/JSON del resultado filtrado (límite de 10.000 filas por exportación).
- **Estadisticas**: tasa de exito, cambios totales, actividad reciente.

### Dashboard y configuracion
- **Dashboard de inicio**: stats (usuarios, grupos, carpetas, cambios), actividad reciente, acciones rapidas.
- **Conexiones NAS independientes**: cada conexión guarda rutas de binarios, volumen de referencia, exclusiones, timeout SSH/keepalive y script previo (VPN). Se aplica al reconectar; exportable sin credenciales ni scripts.
- **Script por conexión**: se ejecuta con tiempo límite de 120 s antes de conectar; si falla, la conexión se bloquea.
- **Configuracion**: tema, retención, gestión de datos y sección **Acerca de** con versión, origen del proyecto y autor.

## Instalacion

### Usuarios finales

Descarga el instalador `.msi` o `.exe` desde [Releases](../../releases). No necesitas instalar Rust ni Node.

### Desarrolladores

**Requisitos previos en Windows:**
- [Node.js](https://nodejs.org/) v18+
- [Rust](https://rustup.rs/) (rustup)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) con el workload "Desktop development with C++"

O ejecuta el script de setup automatico:

```powershell
.\setup.ps1
```

Luego arranca la app en modo desarrollo:

```powershell
npm run tauri dev
```

Para compilar un instalador:

```powershell
npm run tauri build
```

## Uso

1. **Conexiones**: crea una conexion SSH a tu NAS (password o clave SSH)
2. **Dashboard**: revisa el estado general del NAS
3. **Explorador**: navega las carpetas, selecciona multiples, analiza permisos por usuario
4. **Editor ACL**: configura permisos con el arbol visual, preview con dry-run, aplica
5. **Matriz**: genera un grid visual de quien tiene acceso a que
6. **Alta de usuario**: wizard de 3 pasos (datos + grupos + permisos)
7. **Usuarios**: gestiona usuarios y grupos del NAS
8. **Logs**: revisa el historial y deshace cambios

## Arquitectura

```
syno-perm-manager/
├── src-tauri/src/              # Backend Rust
│   ├── ssh/client.rs           # Cliente SSH (russh) - password y clave SSH
│   ├── syno/                   # Wrappers de Synology
│   │   ├── parser.rs           # Parsers de synoacltool, synouser, synogroup
│   │   ├── provider.rs         # Trait AclProvider (para futuro soporte Linux)
│   │   ├── synology.rs         # Implementacion para Synology
│   │   └── utils.rs            # Shell escaping, constantes, helpers
│   ├── db/mod.rs               # SQLite (rusqlite) - conexiones, logs, snapshots, cache
│   ├── commands.rs             # Comandos IPC de Tauri (30+ comandos)
│   ├── models.rs               # Estructuras de datos
│   └── lib.rs                  # Entry point de Tauri
├── src/                        # Frontend Vue 3 + TypeScript
│   ├── views/                  # Dashboard, Conexiones, Explorador, Editor ACL,
│   │                           # Matriz, Wizard, Usuarios, Logs, Configuracion
│   ├── components/             # TreeView, PermissionEditor
│   ├── stores/                 # Pinia (connection, explorer)
│   ├── types/                  # Interfaces TS
│   └── router/                 # Vue Router
├── .github/workflows/          # CI/CD - auto-build de releases
├── setup.ps1                   # Script de setup para desarrolladores
└── LICENSE                     # MIT
```

### Stack tecnologico

| Capa | Tecnologia | Por que |
|---|---|---|
| **Desktop** | Tauri 2 | Binario ligero (~10MB), usa WebView del SO |
| **Backend** | Rust + russh | SSH async puro, sin dependencias externas |
| **BD** | SQLite (rusqlite) | Local, sin servidor, WAL mode |
| **Credenciales** | Windows Credential Manager (keyring) | Credenciales del SO, no en texto plano |
| **Frontend** | Vue 3 + TypeScript + Pinia | Reactivo, tipado, modular |
| **Build** | Vite + cargo | Fast HMR, compilacion optimizada |

## Seguridad

### Modelo de seguridad

La app se conecta al NAS via SSH y ejecuta comandos (`synoacltool`, `synouser`, `synogroup`, `synoshare`). Toda la comunicacion va por canal cifrado SSH. No se exponen puertos, no se instala nada en el NAS.

### Credenciales

| Que | Donde se guarda | Riesgo |
|---|---|---|
| Password SSH | Windows Credential Manager (keyring del SO) | Solo accesible por el usuario de Windows |
| Password de sudo | En memoria durante la sesion, via stdin del canal SSH | No visible en `ps aux` |
| Passphrase de clave SSH | Windows Credential Manager | Solo accesible por el usuario de Windows |
| Clave SSH privada | **No se copia** — solo se guarda la ruta al archivo en disco | El archivo sigue donde lo pusiste |
| Passwords de usuarios creados | No se guardan — se generan, se muestran, y se olvidan | No hay passwords en la BD |
| Logs y snapshots | SQLite local en `%APPDATA%` | Sin passwords, solo metadatos |

### Verificacion de host key (proteccion MITM)

La primera vez que conectas a un NAS, la app guarda el fingerprint SHA256 de su host key en SQLite. En conexiones posteriores, verifica que el fingerprint coincida. Si alguien intercepta la conexion (MITM), la host key sera distinta y la app rechazara la conexion.

### Shell escaping (proteccion inyeccion)

Todos los inputs que van al NAS (paths, usernames, passwords, group names) se escapan con `shell_escape()` usando el metodo POSIX estandar (`'"'"'`). Esto previene inyeccion de comandos via SSH.

### Límites de la protección local

El keyring protege las credenciales en reposo, pero no frente a malware ejecutado como el mismo usuario. La app necesita ejecutar comandos SSH y, si está configurado, `sudo`. Una cadena de comandos unidos por `||` en `authorized_keys` no constituye un despachador seguro ni es compatible con este protocolo. No se incluye un wrapper de comandos restringidos para el NAS.

La primera conexión usa confianza en el primer uso (TOFU); las conexiones posteriores, incluidas las reconexiones automáticas, validan la huella registrada.

### Script pre-conexion (VPN)

La app puede ejecutar un comando antes de conectar al NAS. Util para iniciar una VPN automaticamente:

```
"C:\Program Files (x86)\Sophos\Connect\sccli.exe" enable -n conexion_vpn
```

O comprobar conectividad primero:
```
ping -n 1 192.168.1.100 >nul 2>&1 || (start "" "C:\Program Files (x86)\Sophos\Connect\sccli.exe" & timeout /t 10 /nobreak)
```

El script se guarda en SQLite y se ejecuta con `cmd /C` antes de cada conexion.
La importación de configuraciones omite `startup_script`, tanto en `settings` como en la raíz del JSON. Debe configurarse manualmente en este equipo.

### Revisión de seguridad

Consulta [SECURITY-REVIEW.md](SECURITY-REVIEW.md) para los fallos corregidos, comprobaciones y límites de la revisión. Las nuevas altas ya no incluyen contraseñas en la descripción del usuario. Si usaste versiones anteriores, revisa las descripciones existentes y cambia las contraseñas que hayan quedado expuestas: la actualización no modifica cuentas históricas.

### Auditoria completa

Todas las acciones se registran en SQLite con timestamp, tipo de accion,路径, detalles, y estado (exitos/error):

| Accion | Color en logs |
|---|---|
| `apply_acl` | Azul |
| `restore_snapshot` | Naranja |
| `create_user`, `add_group_member` | Verde |
| `delete_user`, `delete_group`, `rename_*`, `set_*` | Rojo |

Cada cambio de permisos guarda un snapshot de las ACLs anteriores, permitiendo deshacer desde la vista de Logs.

## Comandos de Synology utilizados

| Comando | Uso |
|---|---|
| `synoacltool -get` | Leer ACLs de una carpeta |
| `synoacltool -add` | Anadir entrada ACL (una por comando) |
| `synoacltool -del` | Borrar todas las ACLs de una carpeta |
| `synoshare --enum` | Listar carpetas compartidas |
| `synouser --enum` | Listar usuarios locales |
| `synouser --get` | Detalle de un usuario |
| `synouser --add` | Crear usuario |
| `synouser --del` | Eliminar usuario |
| `synouser --setpw` | Cambiar password |
| `synouser --rename` | Renombrar usuario |
| `synogroup --enum` | Listar grupos |
| `synogroup --get` | Detalle de un grupo |
| `synogroup --descget` | Descripcion de un grupo |
| `synogroup --add` | Crear grupo con miembros |
| `synogroup --del` | Eliminar grupo |
| `synogroup --member` | Establecer miembros |
| `synogroup --memberadd` | Anadir miembro |
| `synogroup --rename` | Renombrar grupo |
| `find -type d` | Listar subcarpetas |

## Roadmap

- [ ] Soporte para cualquier servidor Linux (trait `AclProvider` ya aislado)
- [ ] Plantillas de permisos guardadas y reutilizables
- [ ] Comparar permisos entre 2 NAS
- [ ] Sincronizacion periodica (anti-deriva)
- [x] Tema claro/oscuro
- [ ] Atajos de teclado (Ctrl+Z deshacer, Ctrl+F buscar)
- [ ] Exportar/importar configuracion completa (JSON)

## Licencia

CC BY-NC-SA 4.0 — Copyright (c) 2026 httpbnry

Permite uso y modificacion pero **NO venta comercial**. Si alguien modifica la app, debe compartirla con la misma licencia. Detalles en [creativecommons.org/licenses/by-nc-sa/4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/).
