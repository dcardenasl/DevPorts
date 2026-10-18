# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users
Developers, sobre todo David, que trabajan a diario con varios proyectos y con agentes de IA (Claude Code, Cursor y similares) que levantan servidores de desarrollo. Abren el panel muchas veces al día, en segundos, desde la barra de menús de macOS, para saber qué corre, de qué proyecto es y si algo les pide una decisión.

## Product Purpose
DevPorts mantiene el orden de los servidores de desarrollo: qué corre, de qué proyecto, cuál es el oficial (Principal) y cuáles son pruebas con vencimiento (Temporales). Un CLI (`devports run`) evita que los agentes dupliquen servidores. El éxito es no tener que adivinar qué proceso ocupa cada puerto y no perder tiempo con duplicados.

## Positioning
Es la única herramienta que une la identidad de proyecto declarada (`.devports.yml`), la observación del sistema (procesos, Docker) y la política de agentes (vencimientos, Principal único) en un panel de barra de menús.

## Operating Context
App de barra de menús para macOS (solo v1), panel de 380×520 que se crea al abrir y se destruye al cerrar. Ventana opaca con esquinas redondeadas nativas; no hay transparencia del webview (ADR-025). Tema claro, oscuro o del sistema; español e inglés. Se usa en ráfagas, a veces con la pantalla en cualquier luz.

## Capabilities and Constraints
Cinco vistas (Atención, Proyectos, Compartidos, Sin proyecto, Actividad), hojas para adoptar, declarar compartidos, ignorar, reclasificar y configurar. El texto viene de catálogos tipados (ADR-041); los colores, de tokens por tema (ADR-040) con contraste AA verificado por prueba. Sin `unsafe` nuevo, sin librerías de UI. Solo se detiene automáticamente lo registrado con vencimiento (ADR-006).

## Brand Commitments
Nombre de trabajo «DevPorts» (definitivo en la Fase 4). El color significa estado, nunca decoración (UI-REFERENCIA §6). Sin emojis.

## Evidence on Hand
Capturas de referencia en `docs/ui-referencia/`; el diseño actual en `ui/src/styles.css` y `theme.css`. No hay marca gráfica ni logotipo propios más allá del icono de bandeja.

## Product Principles
1. Simple pero robusto: nada que hoy no se necesite.
2. Lo que pide una decisión se ve primero; lo que ya está bien no pesa.
3. El panel no manda sobre el usuario: confirma antes de detener y explica qué pasará.
4. Coherencia total: el mismo componente se ve igual en todas las vistas.

## Accessibility & Inclusion
Contraste AA en ambos temas (texto 4,5:1; marcadores no textuales 3:1), foco visible, navegación por teclado, `prefers-reduced-motion`, VoiceOver con roles reales. Dos idiomas con textos de distinta longitud.
