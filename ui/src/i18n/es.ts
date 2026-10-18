// Every string the person reads, in Spanish: the source catalog. `en.ts` must have the very
// same shape (the `Catalog` type), so a missing key or a changed signature does not compile.
import type { DocumentKind, FormProblem } from "../types";
import { pluralizer } from "./format";

const { pick, counted } = pluralizer("es");

/** What a sentence of the activity log is written from (see `eventSentence`). */
export interface EventContext {
  /** The service, or «un servicio» / «a service» when the event has none. */
  service: string;
  /** `:8080`, or empty when the event has no port. */
  port: string;
  /** The bare port number, when there is one. */
  portNumber: number | null;
  /** Why a temporary instance was started. */
  reason: string | null;
  /** The agent that launched it, as named by `known_agents`. */
  launcher: string | null;
  /** Where the Principal that refused a launch runs. */
  url: string | null;
  /** The time a lease ends, as the person's clock writes it. */
  expires: string | null;
  /** Whether the run was a temporary one. */
  temporal: boolean;
}

export const es = {
  adoption: {
    review: "Revisar adopción",
    body: "Registra el servidor que ya está corriendo. No lo reinicia.",
    loading: "Consultando destinos…",
    preparing: "Preparando revisión…",
    saving: "Guardando declaración…",
    checking: "Consultando el resultado del guardado…",
    retryQuery: "Consultar de nuevo",
    retry: "Reintentar",
    newReview: "Preparar nueva revisión",
    editAgain: "Volver a editar",
    save: "Guardar",
    continue: "Revisar",
    name: "Nombre del servicio",
    start: "Comando de inicio",
    startHint:
      "Se guarda para futuros lanzamientos. Puedes dejarlo vacío si no hay un comando estable.",
    root: "Raíz del proyecto",
    manifest: "Archivo destino",
    port: "Puerto observado",
    unknownPort: "No disponible",
    creates: "Se creará este archivo al guardar.",
    chooses: "Elige un destino para revisar la propuesta antes de guardar.",
    close: "Cerrar",
    done: "Listo",
    openProject: "Ver proyecto",
    openShared: "Ver compartido",
    pendingCard: "El catálogo todavía no muestra el destino. Espera su actualización.",
    unsent:
      "No se encontró la operación enviada. No se ha reenviado: vuelve a editar y confirma Guardar de nuevo.",
    optionsFailed: "No se pudieron consultar los destinos.",
    targetMissing: "La instancia ya no está disponible para preparar otra revisión.",
  },
  tabs: {
    attention: "Atención",
    projects: "Proyectos",
    shared: "Compartidos",
    orphans: "Sin proyecto",
    activity: "Actividad",
  },
  attention: {
    title: "Atención",
    allClear: "Todo en orden",
    allClearBody: "Nada espera tu decisión.",
    waiting: (count: number) =>
      pick(count, "1 cosa espera tu decisión", `${count} cosas esperan tu decisión`),
    official: "OFICIAL",
    expiring: "TEMPORAL POR VENCER",
    hung: "COLGADA · NO RESPONDE",
    duplicates: (count: number, service: string) => `${count} SIN REGISTRAR · DUPLICAN ${service}`,
    stopAll: (count: number) => `Detener los ${count}`,
    stop: "Detener",
    keep: "Conservar",
    legitimate: "Marcar como legítimo",
    extend: (minutes: number) => `Extender ${minutes} min`,
    terminate: "Terminar",
    seeInProjects: "Ver en Proyectos",
    dismiss: "Ignorar",
    askedBy: (who: string) => `pedido por ${who}`,
    since: (time: string) => `desde ${time}`,
  },
  instance: {
    states: {
      healthy: "Sana",
      unhealthy: "Colgada · no responde",
      down: "Detenido",
      starting: "Iniciando",
      stopping: "Deteniendo",
      forgotten: "Olvidada · sigue activa",
      unknown: "Desconocido · sin observación actual",
    },
    roles: { principal: "Principal", temporal: "Temporal", unregistered: "Sin registrar" },
    details: (name: string) => `Detalles de ${name}`,
    detailsIdentity: (name: string, reference: string) => `Detalles de ${name} · ref ${reference}`,
    state: "Estado",
    role: "Rol",
    origin: "Origen declarado",
    reference: "Referencia",
    pid: "PID",
    identity: "Identidad del objetivo",
    started: "Inicio",
    ports: "Puertos observados",
    noObservedPorts: "Sin puertos observados en la instantánea actual",
    referencePort: "Puerto de referencia",
    folder: "Carpeta",
    worktree: "Worktree",
    branch: "Rama",
    container: "Contenedor",
    compose: "Proyecto Compose",
    image: "Imagen",
    registration: "Registro",
    registered: "Registrada con devports run",
    observed: "Sin registro de devports run",
    noProject: "Sin proyecto conocido",
    unavailable: "No disponible",
    unknownProgram: "Proceso",
    launcher: "Cadena de lanzamiento (informativa)",
    unknownLauncher: "Lanzador no disponible; no se infiere su origen",
    command: "Comando (valores ocultos)",
    hidden: "[oculto]",
    commandPrivacy:
      "Se muestran el ejecutable y las opciones largas. Los valores y argumentos posicionales se ocultan para proteger credenciales y scripts.",
    stale: "Posible código anterior",
    staleBody:
      "Es un indicio basado en fechas de Git, no una prueba de fallo ni una autorización para reiniciar.",
  },
  projects: {
    title: "Proyectos",
    summary: (projects: number, repos: number, services: number) =>
      `${counted(projects, "proyecto", "proyectos")} · ${counted(repos, "repo", "repos")} · ${counted(services, "servicio", "servicios")}`,
    none: "Todavía no hay proyectos",
    noneBody:
      "Un proyecto es una carpeta con un archivo .devports.yml. Adopta un proceso desde «Sin proyecto» para crear el primero.",
    noActive: "sin servicios activos",
    foldAll: "Plegar todo",
    openAll: "Abrir todo",
    unconfirmed: "estado sin confirmar",
    uses: "Usa",
    temporal: "Temporal",
    open: "Abrir en el navegador",
    copy: "Copiar comando de inicio",
    copied: "Comando copiado",
    stop: "Detener",
    stopProject: "Detener servicios del proyecto",
    preparingStop: "Preparando lista…",
    nothingToStop: "No hay aplicaciones activas que se puedan detener en este proyecto.",
    unregistered: "sin registrar",
    duplicatesOf: (count: number) => `${count} sin registrar`,
  },
  shared: {
    title: "Compartidos",
    summary: (count: number) => counted(count, "servicio", "servicios"),
    none: "No hay servicios compartidos",
    onlyUndeclared: "Ninguno declarado todavía",
    noneBody: "Se declaran en ~/.config/devports/shared.yml y los proyectos los citan en «uses».",
    undeclared: "SIN DECLARAR",
    undeclaredBody:
      "Están corriendo y nadie los declaró. Decláralos para que los proyectos los citen.",
    declare: "Declarar",
    declared: "Declarado en shared.yml",
    usedBy: "Lo usan",
    nobody: "Ningún proyecto lo usa",
    data: "datos",
    docker: "docker",
  },
  preferences: {
    appearance: "Apariencia",
    theme: "Tema",
    themes: { system: "Sistema", light: "Claro", dark: "Oscuro" },
    language: "Idioma",
    languages: { system: "Sistema", es: "Español", en: "English" },
    saveFailed: "No se pudo guardar. Se mantiene lo que había.",
  },
  settings: {
    open: "Configuración",
    sheetTitle: "Configuración",
    title: "Carpetas de exploración",
    description:
      "DevPorts busca proyectos (.devports.yml) en estas carpetas. Quitar una solo deja de buscar en ella: no borra archivos ni detiene procesos.",
    loading: "Leyendo la configuración…",
    loadFailed: "No se pudo leer la configuración.",
    retryLoad: "Leer de nuevo",
    notEditable: "No se puede editar desde aquí",
    fromDefaults: "Usas la carpeta por defecto; al guardar se escribirá en config.yml.",
    exists: "existe",
    missing: "no existe ahora",
    projects: (count: number) => counted(count, "proyecto", "proyectos"),
    folder: (index: number) => `Carpeta ${index}`,
    placeholder: "~/developer",
    add: "Añadir carpeta",
    remove: (folder: string) => `Quitar ${folder}`,
    review: "Revisar efecto",
    noChanges: "Sin cambios que guardar",
    reviewTitle: "Revisar efecto",
    reviewBody:
      "Esto cambiará qué proyectos aparecen. No se borra ningún archivo ni se detiene ningún proceso.",
    added: "Empezarán a listarse",
    removed: "Dejarán de listarse",
    nothingChanges: "No cambia la lista de proyectos.",
    save: "Guardar",
    saving: "Guardando configuración…",
    checking: "Consultando el resultado del guardado…",
    retryQuery: "Consultar de nuevo",
    retry: "Reintentar",
    reread: "Volver a leer la configuración",
    unsent:
      "No se encontró la operación enviada. No se ha reenviado: revisa de nuevo y confirma Guardar.",
    saved: "Configuración guardada. Los cambios ya están en uso.",
    advanced: "Los vencimientos y límites avanzados se editan en el archivo:",
    copyPath: "Copiar ruta",
    done: "Listo",
    close: "Cerrar",
  },
  classify: {
    groupProject: "Servidores de desarrollo",
    groupUnclassified: "Sin clasificar",
    groupAuxiliary: "Posibles herramientas auxiliares",
    auxiliaryBody:
      "Suelen ser demonios o programas que lanza otra herramienta. Es una pista, no un hecho: siguen aquí con todas sus acciones y puedes corregir la clasificación.",
    unclassifiedBody: "No hay indicios suficientes para clasificarlos.",
    auxiliaries: (count: number) => counted(count, "posible auxiliar", "posibles auxiliares"),
    action: "Reclasificar",
    title: "Corregir clasificación",
    body: "Solo cambia en qué grupo se lista. No lo ignora, adopta, detiene ni reinicia.",
    current: "Ahora",
    evidence: "Indicios",
    noEvidence: "Sin indicios.",
    scope:
      "La corrección vale solo para esta carpeta y este comando exactos, como una regla de ignorados, y se recuerda al reiniciarlo. Si cambian, vuelve la clasificación automática.",
    choose: "Listar como",
    auto: "Automática",
    autoNow: (label: string) => `Automática (ahora: ${label})`,
    project: "Servidor de desarrollo",
    auxiliary: "Herramienta auxiliar",
    unclassified: "Sin clasificar",
    save: "Guardar",
    corrected: "corregida",
    origin: (origin: string) => `de ${origin}`,
    signals: {
      homebrewDaemon: "Demonio instalado con Homebrew",
      editorExtension: "Programa de una extensión de editor",
      knownProject: "Corre dentro de un proyecto conocido",
      gitWorktree: "Corre dentro de un repositorio Git",
    } as Record<string, string>,
  },
  ignore: {
    action: "Ignorar",
    title: "Ignorar este servicio",
    body: "Deja de mostrarse y de pedir atención. No lo detiene ni lo adopta, y sigue corriendo.",
    criterion: "Se ignorará exactamente esto",
    folder: "Carpeta",
    command: "Comando (valores ocultos)",
    container: "Contenedor",
    context: "Contexto de Docker",
    scope:
      "Solo coincide la misma carpeta y el mismo comando con los mismos argumentos, también tras reiniciarlo o cambiar de pid. Si cambia la carpeta, el comando o un argumento (por ejemplo el puerto), volverá a aparecer.",
    containerScope:
      "Solo coincide el contenedor con ese nombre en ese contexto de Docker, también tras reiniciarlo.",
    restore: "Puedes deshacerlo desde «Ver ignorados».",
    confirm: "Ignorar este servicio",
    loading: "Preparando la regla…",
    failed: "No se pudo preparar la regla.",
    view: "Ver ignorados",
    listTitle: "Servicios ignorados",
    listBody:
      "Cada regla oculta solo lo que describe. Restaurar la quita y el servicio vuelve a observarse; no inicia ni detiene nada.",
    none: "No hay servicios ignorados.",
    legacy: "regla anterior",
    legacyBody:
      "Guardada por una versión anterior: coincide como siempre, sin ampliarse. El comando se muestra aproximado.",
    since: "Desde",
    restoreAction: "Restaurar",
    restoring: "Restaurando…",
    unknown: "Regla que esta versión no sabe leer. Se puede restaurar igualmente.",
    close: "Cerrar",
  },
  consumers: {
    manage: "Gestionar proyectos",
    title: (shared: string) => `Proyectos de ${shared}`,
    description:
      "Marca qué proyectos declaran este recurso en «uses». Es una declaración en sus manifiestos: no detecta conexiones reales ni detiene nada.",
    declaredOnly: "El uso real puede no estar declarado.",
    associated: "Proyectos asociados",
    nobody: "Sin proyectos asociados",
    count: (count: number) => counted(count, "proyecto asociado", "proyectos asociados"),
    search: "Buscar proyecto",
    noMatch: "Ningún proyecto coincide con la búsqueda.",
    loading: "Consultando manifiestos…",
    loadFailed: "No se pudieron consultar los manifiestos.",
    retryLoad: "Consultar de nuevo",
    undefinedShared:
      "Este recurso no está definido en shared.yml. Declara primero el recurso y después asocia proyectos.",
    review: "Revisar cambios",
    noChanges: "Sin cambios que guardar",
    reviewTitle: "Revisar cambios",
    reviewBody:
      "Se editará solo la línea «uses» de estos manifiestos. No se detiene, reinicia ni borra nada.",
    add: "Se añadirá a",
    remove: "Se quitará de",
    save: "Guardar",
    saving: "Guardando manifiestos…",
    checking: "Consultando el resultado del guardado…",
    retryQuery: "Consultar de nuevo",
    retry: "Reintentar pendientes",
    reread: "Volver a leer manifiestos",
    unsent:
      "No se encontró la operación enviada. No se ha reenviado: revisa de nuevo y confirma Guardar.",
    saved: "Guardado",
    pending: "Pendiente",
    partial: "Se guardaron algunos proyectos; los demás siguen pendientes.",
    done: "Listo",
    close: "Cerrar",
    unchanged: "ya estaba así",
    manageFor: (shared: string) => `Gestionar proyectos de ${shared}`,
    associate: (shared: string) => `Asociar ${shared}`,
    stopped: "Detenido",
    copyStart: "Copiar comando de inicio",
    noStart: "Sin comando de inicio declarado en shared.yml.",
    open: "Abrir",
  },
  orphans: {
    title: "Sin proyecto",
    group: (count: number) => counted(count, "proceso", "procesos"),
    otherGroup: "Otros",
    summary: (count: number) =>
      count === 0 ? "Nada suelto" : `${counted(count, "proceso", "procesos")} sin dueño`,
    none: "Nada suelto",
    noneBody: "Todo lo que escucha un puerto pertenece a un proyecto.",
    adopt: "Adoptar",
    seems: (project: string) => `Parece de ${project}`,
    sheetTitle: "¿Dónde va?",
    back: "Volver",
    asService: (project: string) => `Como servicio de ${project}`,
    suggested: "SUGERIDO",
    another: "Otro proyecto",
    anotherBody: "Elegir entre los proyectos conocidos",
    newProject: "Crear un proyecto nuevo",
    newProjectBody: (folder: string) => `Con ${folder} como raíz`,
    shared: "Como servicio compartido",
    sharedBody: "Lo usarán varios proyectos (shared.yml)",
    projectName: "Nombre del proyecto",
    create: "Crear",
    cancel: "Cancelar",
    added: (project: string) => `Agregado a ${project}`,
  },
  activity: {
    title: "Actividad",
    summary: (count: number) =>
      count === 0 ? "Sin actividad en 24 h" : counted(count, "evento", "eventos"),
    of: (shown: number, total: number) => `${shown} de ${counted(total, "evento", "eventos")}`,
    window: "Últimas 24 h",
    loading: "Cargando actividad…",
    unreadable: "No se pudo leer",
    closing: "DevPorts se está cerrando.",
    loadFailed: "No se pudo leer la actividad. Esto no significa que no haya habido ninguna.",
    staleFailed: "No se pudo actualizar la actividad; se muestra la última lectura.",
    retry: "Reintentar",
    noneWindow: "Sin actividad en las últimas 24 h",
    noneWindowBody:
      "Aquí aparecen los arranques, rechazos, vencimientos y avisos de tus servidores.",
    noMatch: (total: number) =>
      `Ningún evento coincide con este filtro (hay ${counted(total, "evento", "eventos")} en total).`,
    showAll: "Ver todo",
    all: "Todo",
    rejections: "Rechazos",
    temporals: "Temporales",
    alerts: "Alertas",
    none: "Sin actividad en este filtro",
    unnamedService: "un servicio",
    events: {
      core_started: () => "DevPorts se inició",
      run_accepted: (e: EventContext) =>
        e.temporal
          ? `Prueba de ${e.service} ${e.port} para «${e.reason ?? "sin motivo"}»`
          : `${e.service} registrada como Principal ${e.port}`,
      run_rejected: (e: EventContext) =>
        `${e.launcher === null ? "un proceso" : `un agente (${e.launcher})`} intentó levantar otra instancia de ${e.service}; la Principal sigue ${e.url === null ? "" : `en ${e.url}`}`,
      reuse: (e: EventContext) =>
        `${e.launcher === null ? "un proceso" : `un agente (${e.launcher})`} reutilizó la Principal de ${e.service}`,
      reservation_released: (e: EventContext) =>
        `Se liberó el puerto reservado para ${e.service}: nadie lo abrió a tiempo`,
      bound: (e: EventContext) => `${e.service} ${e.port} quedó vinculada a su proceso`,
      port_stolen: (e: EventContext) =>
        `Otro proceso tomó el puerto ${e.portNumber ?? "?"} reservado para ${e.service}`,
      lease_warned: (e: EventContext) => `La prueba de ${e.service} ${e.port} vence en 1 minuto`,
      lease_extended: (e: EventContext) =>
        `La prueba de ${e.service} ${e.port} se extendió; vence ${e.expires ?? "más tarde"}`,
      lease_expired: (e: EventContext) => `La prueba de ${e.service} ${e.port} venció y se detuvo`,
      stopped: (e: EventContext) => `${e.service} ${e.port} se detuvo`,
      stop_failed: (e: EventContext) =>
        `${e.service} ${e.port} no se pudo detener: queda como olvidada`,
      duplicate_detected: (e: EventContext) =>
        `Apareció un duplicado de ${e.service} ${e.port} sin registrar`,
      kept: (e: EventContext) => `Conservaste ${e.service} ${e.port} aunque duplica a la Principal`,
      restored: () => "Restauraste un servicio ignorado",
      reclassified: (e: EventContext) => `Cambiaste la clasificación de ${e.service} ${e.port}`,
      adopted: (e: EventContext) => `Adoptaste ${e.service} en un manifiesto`,
      unregistered_seen: (e: EventContext) => `Apareció un proceso sin registrar ${e.port}`,
      process_gone: (e: EventContext) => `${e.service} ${e.port} dejó de ejecutarse`,
    },
    labels: {
      rejection: "RECHAZO",
      temporal: "TEMPORAL",
      primary: "PRINCIPAL",
      warned: "AVISO",
      extended: "EXTENDIDO",
      expired: "VENCIÓ",
      unregistered: "SIN REGISTRAR",
      adopted: "ADOPTADO",
      kept: "CONSERVADO",
      restored: "RESTAURADO",
      reclassified: "CLASIFICADO",
      stopped: "DETENIDO",
      alert: "ALERTA",
    },
  },
  operations: {
    title: "Operaciones recientes",
    kind: {
      stop: "Detener",
      stopProject: "Detener proyecto",
      extend: "Extender",
      keep: "Ignorar",
      restore: "Restaurar",
      classify: "Clasificación",
      setScanFolders: "Carpetas de exploración",
      adopt: "Adoptar",
      setConsumers: "Proyectos asociados",
    },
    queued: "En espera",
    running: "En curso…",
    succeeded: "Completado",
    retry: "Reintentar fallidos",
    failed: "Falló",
    adopted: (project: string, service: string) => `Agregado a ${project} · ${service}`,
  },
  confirm: {
    stopProjectTitle: (project: string) => `¿Detener aplicaciones de ${project}?`,
    projectStopBody:
      "Solo se detendrán las instancias enumeradas. Los datos y compartidos seguirán activos. Los nuevos lanzamientos quedan fuera de esta acción.",
    stopTitle: (name: string) => `¿Detener ${name}?`,
    stopBody: "Se le pedirá que termine y, si no lo hace, se forzará.",
    stopGroupTitle: (count: number) => `¿Detener estas ${count} instancias?`,
    dataStopBody:
      "Se detendrá el contenedor de datos. DevPorts no borrará el contenedor ni sus volúmenes.",
    affects: "Perderán este servicio:",
    identity: (view: {
      pid: number | null;
      container: string | null;
      ref: string;
      started_at: string | null;
    }) =>
      view.pid !== null
        ? `Proceso pid ${view.pid}${view.started_at === null ? "" : ` · iniciado ${view.started_at}`}`
        : `Contenedor ${view.container ?? "—"} · ref ${view.ref}`,
    port: (port: number | null) => (port === null ? "Sin puerto" : `Puerto ${port}`),
    origin: (view: { shared: boolean; project: string | null; service: string | null }) =>
      view.shared
        ? "Recurso compartido"
        : view.project === null
          ? "Sin proyecto"
          : `${view.project}${view.service === null ? "" : ` · ${view.service}`}`,
    targetChanged: "La instancia cambió desde la confirmación. Revísala y confirma de nuevo.",
    cancel: "Cancelar",
    stop: (name: string) => `Detener ${name}`,
    stopGroup: "Detener seleccionados",
  },
  status: {
    dockerDown: "Docker no responde",
    dockerDownBody:
      "Los contenedores aparecen como «sin confirmar», no como detenidos. Abre Docker Desktop: DevPorts vuelve a comprobarlo solo.",
    draftGone:
      "El proceso de este borrador ya no está disponible. Se descartó el formulario; no se realizó ninguna acción.",
    problems: "Avisos del núcleo",
    connecting: "Conectando con el núcleo…",
    loading: "Cargando la información de proyectos y puertos…",
    failed: "No se pudo completar la acción",
  },
  link: {
    connectingAttempt: (attempt: number, total: number) =>
      `Conectando con DevPorts… intento ${attempt} de ${total}`,
    cannotConnect: "No se pudo conectar",
    cannotConnectBody:
      "El panel no consigue hablar con DevPorts. Comprueba que la app sigue abierta (icono en la barra de menús) y reintenta. Las operaciones que ya confirmaste no se reenvían.",
    reconnecting: (attempt: number, total: number, since: string) =>
      `Sin conexión con DevPorts · reintentando (${attempt}/${total}). Estos datos son de ${since}.`,
    lost: (since: string) =>
      `No se pudo reconectar. Estos datos son de ${since} y las acciones pueden fallar.`,
    retry: "Reintentar",
    retryNow: "Reintentar ahora",
    problems: (count: number) =>
      pick(
        count,
        "1 aviso de configuración o manifiestos",
        `${count} avisos de configuración o manifiestos`,
      ),
    details: "Ver detalle",
    state: "Estado",
    core: "Núcleo",
    docker: "Docker",
    coreOk: "Conectado",
    coreRetrying: "Sin conexión; reintentando",
    coreLost: "Sin conexión",
    dockerOk: "Responde",
    dockerDown: (reason: string) => `No responde: ${reason}`,
    noProblems: "Sin avisos de configuración ni de manifiestos.",
    problemsTitle: "Avisos",
  },
  errors: {
    settings: {
      invalidRoots: "Hay carpetas que no se pueden usar. Corrígelas y vuelve a revisar.",
      documentChanged: (path: string) =>
        `${path} cambió desde que lo revisaste. Vuelve a leer la configuración antes de guardar.`,
    },
    adoption: {
      documentChanged: (path: string) =>
        `${path} cambió. Prepara una nueva revisión antes de guardar.`,
      targetChanged: "La instancia o su destino cambiaron. Vuelve a revisar la propuesta.",
      previewExpired: "La revisión caducó. Tus campos se conservan; prepara otra revisión.",
      requestChanged:
        "Esta revisión ya pertenece a otro guardado. Consulta su resultado o prepara otra revisión.",
      atCapacity: "Hay demasiadas operaciones en curso. Espera y vuelve a intentarlo.",
    },
    uses: {
      unknownShared: (name: string) =>
        `«${name}» ya no está definido en shared.yml. Decláralo antes de asociarlo.`,
      unknownProject: (project: string) => `El proyecto «${project}» ya no existe.`,
      documentChanged: (path: string) =>
        `${path} cambió desde que lo revisaste. Consulta de nuevo antes de guardar.`,
      blocked: "No se escribió nada: otro proyecto de esta selección no pasó la validación.",
      invalidRequest: "La selección repite un proyecto o supera el máximo.",
    },
  },
  reasons: {
    io: (detail: string) => `No se pudo leer o escribir: ${detail}`,
    closing: "DevPorts se está cerrando.",
    action: {
      unknownInstance: (reference: string) => `No existe la instancia «${reference}».`,
      notRunning: (reference: string) => `La instancia «${reference}» no está en marcha.`,
      targetChanged: "La instancia cambió desde la confirmación; revísala de nuevo.",
      noKnownFolder: "No se puede recordar un proceso sin carpeta conocida.",
      tooManyCorrections: "Hay demasiadas correcciones de clasificación; restablece alguna.",
      storage: (detail: string) => `No se pudo leer o guardar: ${detail}`,
      internal: (detail: string) => `Error interno: ${detail}`,
    },
    stop: {
      protected: (pid: number) => `El proceso ${pid} está protegido (DevPorts o su entorno).`,
      otherUser: (pid: number) => `El proceso ${pid} pertenece a otro usuario.`,
      stillAlive: "La instancia sigue activa tras intentar detenerla.",
      processTableUnreadable: "No se pudo leer la tabla de procesos.",
      dockerUnavailable: "Docker no está disponible.",
      failed: (detail: string) => detail,
    },
    command: {
      tooManyTargets: (max: number) =>
        `Hay demasiadas instancias para una sola operación (máximo ${max}); detén servicios individualmente.`,
      onlyLocalUrls: "Solo se abren direcciones locales.",
      invalidPanelState: (detail: string) => `El contexto del panel no es válido: ${detail}`,
      operationInvalid: "La solicitud de operación no es válida.",
      operationIdConflict: "Ese identificador ya pertenece a otra operación.",
      operationAtCapacity: "Hay demasiadas operaciones en curso.",
      operationNotFound: "La operación no existe.",
      nothingToRetry: "La operación no tiene objetivos fallidos que reintentar.",
    },
    ineligibility: {
      configInvalid: (detail: string) =>
        `config.yml tiene errores; corrígelos a mano antes de editar: ${detail}`,
      noManifest: "El proyecto aún no tiene .devports.yml; adopta primero uno de sus servicios.",
      manifestInvalid: (detail: string) => `El manifiesto no es válido: ${detail}`,
      readOnly: (path: string) => `${path} es de solo lectura.`,
      rootsNotEditable:
        "config.yml escribe «roots» en una forma que DevPorts no edita; edítalo a mano.",
    },
    limitation: {
      notRegularFile: "Solo se editan archivos regulares, sin enlaces simbólicos.",
      hardLinked: "No se reemplazan manifiestos con enlaces físicos.",
      notUtf8: "El archivo no es texto UTF-8.",
      tooLarge: (max: number) => `La edición automática admite como máximo ${max} bytes.`,
      ownerNotKept: "No se puede conservar el propietario o grupo del archivo.",
      targetNotAbsolute: "El destino debe ser absoluto y no contener «..».",
      noTargetFile: "Falta el archivo destino.",
      noTargetFolder: "Falta la carpeta destino.",
      targetFolderNotDirectory: "La carpeta destino no es un directorio regular.",
      targetFolderNotFound: "No se encontró la carpeta destino.",
      targetFolderUnresolvable: "No se puede resolver la carpeta destino.",
      form: (key: string, problem: FormProblem) => {
        const name = `«${key}»`;
        switch (problem) {
          case "notFound":
            return `No se encontró ${name}.`;
          case "notAList":
            return `${name} no es una lista; edítalo a mano.`;
          case "notSimpleList":
            return `${name} no es una lista de valores simples; edítalo a mano.`;
          case "multiLine":
            return `${name} abarca varias líneas; edítalo a mano.`;
          case "mixedIndent":
            return `${name} mezcla sangrías; edítalo a mano.`;
          case "emptyItem":
            return `${name} tiene un elemento vacío.`;
          case "invalidValue":
            return `${name} contiene valores que no son simples; edítalo a mano.`;
          case "notEditable":
            return `${name} está escrito en una forma que DevPorts no edita; edítalo a mano.`;
        }
      },
      editInvalid: (document: DocumentKind, detail: string) =>
        `La edición no produjo ${document === "config" ? "una configuración válida" : "un manifiesto válido"}: ${detail}`,
      editTouchedMore: (key: string) =>
        `La edición habría cambiado algo más que «${key}»; edítalo a mano.`,
      other: (detail: string) => detail,
    },
    unsuitability: {
      instanceUnavailable: "Esa instancia no está disponible para adopción.",
      sharedNeedsContainer: "Solo los contenedores pueden declararse como compartidos.",
      containerNeedsProject: "Para ese contenedor elige un proyecto existente o Compartidos.",
      folderUnresolvable: (detail: string) => `No se puede resolver la carpeta: ${detail}`,
      noRoot: "El destino no tiene raíz.",
      noKnownFolder: "El proceso no tiene carpeta conocida.",
      folderTooBroad:
        "Esa carpeta es demasiado amplia para ser un proyecto; adopta el proceso desde la carpeta de su repositorio.",
      manifestExists: "Ya hay un manifiesto en la raíz; elige ese proyecto y revisa su contenido.",
      outsideProjectRoot: "La carpeta del proceso está fuera de la raíz del proyecto.",
      outOfScanScope:
        "Esta raíz no está en el alcance de exploración; añade su carpeta en Configuración antes de adoptar.",
      projectNotInCatalog: "Ese proyecto ya no está en el catálogo.",
      resultTooLarge: "El resultado supera el límite de edición automática.",
      notDescribable: "No se pudo describir ese proceso o contenedor desde esa carpeta.",
      fileInvalid: (detail: string) => detail,
      other: (detail: string) => detail,
    },
    field: {
      invalidName: "Usa de 1 a 63 letras minúsculas, números o guiones.",
      invalidStart: "El comando debe tener una sola línea, sin NUL, entre 1 y 16384 bytes.",
      nameTaken: "Ya existe un servicio con ese nombre.",
      tooLong: "El valor supera el límite permitido.",
    },
    listProblem: {
      empty: "Deja al menos una carpeta: sin ellas no se descubriría ningún proyecto.",
      tooMany: (max: number) => `Como máximo ${max} carpetas.`,
    },
    rootProblem: {
      empty: "escribe una carpeta",
      unsafeText:
        "la ruta contiene caracteres que no se admiten (comillas, barras invertidas o saltos de línea)",
      notAbsolute: "usa una ruta absoluta o que empiece por ~/",
      tooBroad: "esa carpeta es demasiado amplia; elige la que contiene tus proyectos",
      duplicate: "esa carpeta ya está en la lista",
      missing: "la carpeta no existe",
    },
    reviewNote: {
      launcherProgram: (program: string) =>
        `«${program}» suele lanzar el servidor real como proceso hijo: revisa el comando.`,
      notOneLine:
        "El comando observado no se representa como una línea; indica un comando o script estable.",
      noStartCommand:
        "No se pudo proponer un comando de inicio; puedes declararlo sin inicio automático.",
      manifestWarning: (detail: string) => detail,
    },
  },
  a11y: {
    operations: "Operaciones",
    sections: "Secciones",
    filter: "Filtro",
  },
};

export type Catalog = typeof es;
