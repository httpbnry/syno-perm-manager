# Revisión de seguridad y validación

Fecha: 2026-10-06 / actualización 2026-10-07. Alcance: revisión del código local de Vue/Tauri/Rust y auditoría de dependencias. No se ha conectado un NAS real durante esta revisión.

## Correcciones

| Hallazgo | Corrección |
|---|---|
| Las altas añadían la contraseña a `full_name`, persistiendo el secreto en el NAS | Eliminado en ambos formularios; retirada la variable `{password}` de la plantilla de descripción |
| Contraseñas generadas con `Math.random()` | Generador común con Web Crypto y muestreo sin sesgo |
| Valores ACL interpolados sin escape en comandos SSH | Escape POSIX del argumento completo al aplicar y restaurar |
| La reconexión SSH aceptaba cualquier clave de host | Reutiliza la huella autenticada de la sesión; fallos al persistir la huella se notifican |
| Los comandos con sudo se repetían cuando terminaban con error | Una única ejecución; se evita repetir modificaciones parcialmente aplicadas |
| Cierre de canal sin estado tratado como éxito | Estado inicial de error hasta recibir `ExitStatus` |
| Lecturas ACL fallidas convertidas en listas vacías | Se propagan errores en lecturas, previsualización, snapshots recursivos y análisis en lote |
| Restauración ignoraba errores al borrar ACL | Detención de esa carpeta ante error de borrado |
| Snapshots restaurables en otra conexión | Comprobación de la conexión propietaria del snapshot |
| Configuraciones importadas podían instalar scripts ejecutables automáticamente | Se omiten scripts de inicio importados en ambos formatos |
| CSP deshabilitada | Políticas explícitas para producción y desarrollo |
| Fórmulas en exportaciones CSV | Neutralización de prefijos interpretables como fórmulas y escape de comillas |
| Caché sin caducidad ni invalidación tras cambios | TTL de cinco minutos e invalidación antes de escrituras ACL |
| Rutas NAS globales para todos los equipos | Perfiles validados por conexión, con migración única desde ajustes legacy |
| Scripts locales sin límite de ejecución | Timeout de 120 s, ejecución asíncrona y bloqueo de conexión si falla el script del perfil |
| Historial sin paginación ni detalle perezoso | Consultas filtradas en servidor, detalle bajo demanda y exportaciones acotadas |

## Comparación de usuarios

- El backend crea y conserva el plan: el frontend solo puede aplicar su identificador, no enviar comandos.
- Un solo plan por sesión, de un solo uso, con caducidad de 15 minutos; se invalida al conectar o desconectar.
- Se releen todos los recursos de la vista previa antes de la primera modificación. Si cambian, se exige una nueva comparación.
- Borrado por el índice real de cada ACE, en orden descendente; se conservan las ACE de otros sujetos y las heredadas.
- Procesamiento de carpetas hijas antes de sus padres, para evitar que la propagación de herencia cambie índices pendientes.
- Resolución canónica y deduplicación de rutas; exclusión de carpetas de metadatos y papelera en el recorrido.
- Se bloquea la retirada del grupo primario del destino.
- El estado previo se registra antes de escribir. Los respaldos `user_sync_backup` son exportables desde el historial y requieren recuperación manual.
- Se detiene la aplicación en el primer error. Un fallo de transporte puede dejar incierto el resultado del último comando: vuelve a comparar.

## Dependencias

- `npm audit`: inicialmente seis dependencias señaladas; corregidas con actualizaciones compatibles del lockfile. Resultado actual: **0 vulnerabilidades**.
- `cargo audit`: se revisó el lockfile con RustSec. Se actualizaron `event-listener` a 5.4.2 (corrige RUSTSEC-2026-0221) y las versiones retiradas de `chacha20`, `der` y `wnaf`.
- La revisión final deja **7 avisos**: seis de mantenimiento de `proc-macro-error` y la familia `unic-*`, y el aviso de seguridad de memoria de `glib` 0.18.5 (RUSTSEC-2024-0429, dependencia del backend Linux/GTK). Consultar la salida actual de `cargo audit` antes de distribuir para otras plataformas.

## Verificación

- `npm run build`: comprobación de tipos y compilación de producción.
- `cargo test --lib --locked`: diez pruebas (comparación, perfiles NAS aislados, historial filtrado, rutas multovolumen y escape ACL).
- `git diff --check`: integridad del diff.

## Límites y validación pendiente en DSM

No se afirma ausencia total de vulnerabilidades. La auditoría de dependencias depende de los avisos publicados. Las credenciales permanecen en memoria durante la sesión; `synouser` recibe las contraseñas como argumentos de su CLI. El script local configurado manualmente sigue ejecutándose con los permisos del usuario del equipo.

Las descripciones de cuentas creadas anteriormente pueden contener contraseñas: deben revisarse y rotarse en el NAS. La primera conexión SSH sigue usando TOFU. Los snapshots del editor ACL preexistente usan su formato anterior y no equivalen al respaldo completo de la nueva comparación.

La comprobación previa reduce cambios sobre datos obsoletos, pero SSH no ofrece transacciones ni bloqueo frente a administradores externos en DSM. La sincronización no modifica propietarios, grupos primarios, ACL específicas de archivos, permisos de aplicaciones DSM ni pertenencias de directorios externos. Los permisos heredados deben reproducirse desde su carpeta de origen.

Antes del despliegue, validar con dos usuarios de prueba en la versión real de DSM: grupos adicionales, allow/deny, permisos heredados, rutas con espacios, distintos volúmenes, repetición sin cambios y un fallo de acceso controlado. Comprobar desde DSM y desde una sesión nueva del usuario destino, ya que las sesiones SMB abiertas pueden conservar grupos anteriores.
