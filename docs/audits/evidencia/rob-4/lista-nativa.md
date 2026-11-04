# ROB-4.4 — Lista de comprobación nativa (la hace David)

Con la app instalada (`tools/install.sh`). Marca cada punto y anota lo que veas; el resultado va a `ARCHIVES.md` (ROB-4.4).

## Tema y tamaño
- [ ] **Claro y oscuro** en cada pestaña (Atención, Proyectos, Compartidos, Sin proyecto, Actividad) y en Ajustes: sin texto ilegible ni bordes que desaparezcan.
- [ ] La **rueda de Ajustes**, los **filtros de Actividad** (Todo, Rechazos, Temporales, Alertas) y los **segmentos de Apariencia** se pulsan sin apuntar con cuidado (área de 32 px, ROB-4.1).

## Teclado
- [ ] Con solo teclado: pestañas con flechas, Inicio y Fin; Tab recorre botones en orden; Esc cierra hojas y diálogos y devuelve el foco a quien los abrió.
- [ ] El anillo de foco se ve en ambos temas.

## VoiceOver (Cmd+F5)
- [ ] Atención: cada fila anuncia nombre, estado y puerto; «Detener los N» anuncia a qué instancias afecta.
- [ ] Diálogo «Detener»: anuncia qué instancia **se conserva** (ROB-4.2) y mueve el foco a «Cancelar».
- [ ] Adoptar: un destino no disponible anuncia **su motivo** (ROB-4.3), no solo «deshabilitado».
- [ ] Las insignias de las pestañas se leen como cantidad pendiente. *(Conocido: el texto accesible de la insignia está fijo en español; ver `tabs.tsx`.)*

## Casos de datos
- [ ] Un servidor reiniciado con `localhost` en vez de `127.0.0.1` sigue en su servicio (no aparece en «Sin proyecto»).
- [ ] Un proceso en una carpeta con `.devports.yml` fuera de `~/developer` muestra el aviso de carpeta no buscada (cuando exista ROB-5.3).
