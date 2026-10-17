# Resolución de la auditoría UI/UX — entrevista grill-me

Fuente: [auditoría](2026-10-17-ui-ux.md). Esta entrevista define soluciones; no implementa cambios. Registrar por separado decisiones confirmadas y propuestas pendientes.

## Decisiones confirmadas

### D1 — Todos los flujos dentro del panel
David prefiere mantener todas las operaciones dentro del panel de la barra. Al hacer clic fuera se cierra, conforme al comportamiento esperado de este tipo de elemento. No añadir una ventana de gestión separada para adopción, recursos compartidos o configuración. Registrado también como ADR-028.

### D2 — Recuperar borradores durante la ejecución de la app
David acepta recuperar el borrador al reabrir el panel mientras la app siga abierta. Cerrar no guarda cambios. Guardar es explícito y revalida los datos actuales. Las confirmaciones de parada se solicitan de nuevo, no se restauran como autorizaciones pendientes. No se ha acordado persistencia de borradores tras salir o reiniciar la app.

### D3 — Confirmar las paradas manuales
David acepta confirmación breve para toda parada manual, con identidad, puerto y origen. En compartidos se muestran los consumidores declarados; en datos se explica qué se detendrá. Las comprobaciones de identidad del núcleo siguen siendo obligatorias y no se amplían los permisos del CLI.

### D4 — Cerrar un proyecto conserva los recursos compartidos
David establece que cerrar un proyecto no detiene recursos compartidos como MySQL, que pueden seguir alimentando otros proyectos. No hay parada en cascada desde el proyecto a sus compartidos. Detener el recurso compartido es una acción independiente con confirmación y consumidores declarados visibles. El alcance de la acción por proyecto queda concretado en D5 y D14.

### D5 — Parada conjunta de servidores de un proyecto
David acepta añadir «Detener servicios del proyecto»: incluye servidores de aplicación del proyecto y sus Temporales; excluye recursos compartidos y servicios `kind: data`. Antes de ejecutar se muestra exactamente qué se detendrá y se solicita confirmación. No elimina manifiestos ni desasocia recursos. Pendiente: concretar en implementación el tratamiento de identidad, instancias que cambien mientras el diálogo está abierto y resultados parciales.

### D6 — Declarar compartidos y seleccionar consumidores opcionales
David acepta la recomendación: al declarar un recurso compartido se ofrece «¿Qué proyectos lo usan?» con selección múltiple, editable después. Se permite no seleccionar ninguno; el estado se describe como «Sin proyectos asociados», sin afirmar que no hay consumidores reales. La asociación es declarativa mediante `uses`, no una detección de conexiones.

### D7 — Guardado de asociaciones con resultados por proyecto
David acepta validar todos los manifiestos antes de comenzar. Si se conoce un error en esa validación, no iniciar escrituras. Si un fallo ocurre durante el guardado, informar resultados por proyecto, conservar la selección y permitir reintentar solo pendientes. No prometer atomicidad global ni revertir automáticamente archivos que podrían haber recibido cambios externos. Los éxitos deben reflejar el estado realmente persistido y el resumen no debe declarar éxito total cuando hay pendientes.

### D8 — Revisión obligatoria antes de adoptar
David acepta la recomendación: adoptar siempre tiene dos pasos dentro del panel, elegir destino y revisar/guardar. La revisión muestra raíz real, archivo destino y puerto observado, y permite editar nombre del servicio y comando de inicio. Incluye advertencias cuando la propuesta no es fiable. Guardar declara el servidor existente; no lo reinicia. No se adopta inmediatamente por la ausencia de advertencias.

### D9 — Ignorados persistentes y restaurables
David acepta mantener la exclusión cuando el servicio vuelva a arrancar, con acción «Ignorar este servicio», explicación del alcance y acceso «Ver ignorados → Restaurar» dentro del panel. Ignorar no detiene ni adopta. El detalle del criterio de coincidencia debe ser explícito y no ampliarse silenciosamente.

### D10 — Auxiliares separados sin exclusión automática
David acepta un grupo «Posibles herramientas auxiliares», plegado inicialmente, con origen visible al expandir. La clasificación es orientativa; no ignora ni oculta permanentemente procesos por su cuenta. Mantener acceso a los elementos y posibilidad de corregir clasificaciones equivocadas; una clasificación no autoriza por sí sola una parada.

### D11 — Recuperar el contexto de navegación
David acepta volver a la última pestaña, proyecto expandido y filtro al reabrir el panel. Se recupera también el borrador según D2. Las alertas nuevas se anuncian en el contador de Atención sin cambiar de sección automáticamente. La persistencia mínima debe vivir fuera del webview efímero; no se ha aprobado conservar este contexto después de salir o reiniciar la app.

### D12 — Las operaciones confirmadas continúan con el panel cerrado
David acepta que Guardar o Detener ya confirmados continúen al cerrar el panel. Al reabrir se muestra el progreso o el resultado real (éxito, fallo o parcial) sin volver a ejecutar la operación. El seguimiento de la acción debe sobrevivir a la destrucción del webview; una confirmación aún no enviada no constituye una operación autorizada pendiente. Esta decisión no establece recuperación después de salir o reiniciar la app.

### D13 — Configuración acotada dentro del panel
David acepta gestionar carpetas de proyectos, errores de configuración y estado del núcleo/Docker dentro del panel. Los tiempos de vencimiento, límites de Temporales y demás opciones avanzadas siguen en YAML por ahora. Quitar una carpeta de exploración no implica borrar el proyecto ni detener procesos; al implementar se debe explicitar ese alcance.

### D14 — Detener un proyecto es una operación puntual
David acepta actuar solo sobre los servidores incluidos en la lista confirmada. No introduce un estado persistente de proyecto cerrado ni bloquea que una persona o un agente lance otro servidor después. Los nuevos procesos siguen la observación y política habituales.

## Observaciones técnicas durante la entrevista
- `Engine::ignore_key` usa cwd, programa y argv concatenado para procesos; para contenedores usa el nombre. El comentario afirma resistencia a cambios de puerto, pero argv incluye los argumentos crudos: cambiar un puerto escrito en argv puede cambiar la clave. Al resolver UX-06, probar el alcance real, evitar colisiones de argumentos concatenados y considerar el contexto Docker en identidad. La aceptación de persistencia no aprueba una normalización amplia que oculte otros servicios.
- `Store::remove_ignored` ya existe: falta exponer una operación humana segura y UI de restauración; no hace falta inventar otra persistencia.

## Cierre de la entrevista
Las decisiones de producto D1–D14 están confirmadas. Los detalles técnicos pendientes se resuelven con las restricciones existentes y pruebas, sin presentar propuestas técnicas como decisiones nuevas de David. Persistencia de borradores después de salir de la app, bloqueo de lanzamientos por proyecto y edición visual de opciones avanzadas no forman parte de este plan.

La secuencia de trabajo y aceptación está en [plan de corrección](2026-10-17-ui-ux-plan.md). Las propuestas originales de la auditoría se leen junto con estas decisiones: todo permanece en el panel, toda parada manual se confirma y no hay parada en cascada de recursos compartidos.
