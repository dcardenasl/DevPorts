---
name: DevPorts
description: Panel de barra de menús que se siente de sistema: listas agrupadas, un solo canal de color para el estado y tipografía SF con jerarquía real.
colors:
  panel-dark: "#1c1c1e"
  group-dark: "#2c2c2e"
  text-dark: "#f5f5f7"
  text-soft-dark: "#acacb1"
  text-dim-dark: "#a3a3a8"
  accent-dark: "#5aacff"
  accent-fill-dark: "#0071e3"
  green-dark: "#32d74b"
  amber-dark: "#ff9f0a"
  red-dark: "#ff7b73"
  panel-light: "#f2f2f7"
  group-light: "#ffffff"
  text-light: "#1d1d1f"
  text-soft-light: "#636366"
  text-dim-light: "#66666b"
  accent-light: "#0066cc"
  accent-fill-light: "#0071e3"
  green-light: "#1e7e34"
  amber-light: "#a15c00"
  red-light: "#c4271c"
typography:
  title:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', system-ui, sans-serif"
    fontSize: "20px"
    fontWeight: 700
    lineHeight: 1.2
  headline:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 600
    lineHeight: 1.3
  body:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.35
  label:
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 600
    lineHeight: 1.3
  data:
    fontFamily: "ui-monospace, 'SF Mono', Menlo, monospace"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: 1.3
rounded:
  control: "6px"
  group: "10px"
  sheet: "12px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
components:
  group:
    backgroundColor: "{colors.group-dark}"
    rounded: "{rounded.group}"
  row:
    padding: "8px 12px"
    height: "44px"
  button-primary:
    backgroundColor: "{colors.accent-fill-dark}"
    textColor: "#ffffff"
    rounded: "{rounded.control}"
    height: "28px"
    padding: "0 12px"
---

# Design System: DevPorts

## Overview

**Creative North Star: «El panel de sistema».** DevPorts es una utilidad de barra de menús que se abre diez veces al día, tres segundos cada vez. Tiene que parecer parte de macOS, no una web: listas agrupadas como Ajustes del Sistema, separadores finos, tipografía SF y casi nada de color. Quien lo abre debe saber qué corre y qué le pide una decisión sin leer.

El panel es **Operate** (ver PRODUCT.md): la familiaridad es una virtud, la expresión se guarda para detalles precisos. Qué rechaza: una tarjeta con borde y fondo teñido por cada elemento, color de estado en texto, borde, fondo y botón a la vez, etiquetas en mayúsculas monoespaciadas, y fundidos de entrada en cada vista.

Restricción técnica: la ventana es opaca (ADR-025). No hay vibrancy; el material se imita con neutros planos y separadores de un píxel.

## Colors

Estrategia **Restrained**: neutros más un acento, y el color de estado en un solo canal.

- **Neutros (panel, group, sheet):** el panel es el fondo; el *group* es el contenedor de filas, un paso más claro en oscuro y blanco sobre gris en claro. Las hojas usan el mismo tono que el group.
- **Acento (`accent`, `accent-fill`):** la única acción primaria, el foco y los enlaces. `accent-fill` es el relleno del botón primario (blanco encima, ≥ 4,5:1); `accent` es el texto y el aro de foco (≥ 4,5:1 sobre panel y group).
- **Estado (`green`, `amber`, `red`):** solo el **punto de estado** y, cuando algo pide acción, la palabra de estado. Nunca fondo, borde, relleno de botón ni etiqueta teñida. El azul de datos desaparece: los datos se distinguen por texto, no por color.
- **Reglas:** el color significa estado; un elemento sano es neutro. Rojo solo para lo que falla o se detiene; ámbar para lo que vence; verde para el Principal sano.

## Typography

Una familia, la del sistema (SF), con una escala corta: título 20/700 por vista, titular 13/600 para el nombre de una fila, cuerpo 13/400, etiqueta 11/600 en minúscula normal sin tracking para encabezados de grupo, y dato 12/500 monoespaciada solo para puertos, rutas, comandos y cuentas regresivas (cifras tabulares). Jerarquía por peso y tamaño, no por mayúsculas ni color. Medida de prosa ≤ 65 caracteres; los textos largos envuelven.

## Layout

Una columna de 380 px con márgenes de 12. Cada sección es **un grupo**: una superficie redondeada con filas separadas por una línea de 1 px que empieza donde empieza el texto (no en el borde). Entre grupos, 16 px; entre filas, 0 (las separa la línea). Fila de dos líneas = 44 px mínimo; de una línea = 32 px. Encabezado de grupo: etiqueta 11/600 gris, 6 px sobre el grupo. Nada de grupos dentro de grupos.

## Elevation & Depth

Plano. La profundidad es tonal (panel → group → sheet) más el separador de 1 px. Las hojas llevan un velo, no sombra. Sin halos de color ni sombras de sistema en componentes.

## Shapes

Tres radios: 6 (controles, campos, botones), 10 (grupos), 12 (hojas y avisos). Los puntos de estado son círculos de 8 px; el anillo vacío significa «sin proceso observado» y el discontinuo «sin dueño».

## Components

- **Fila (`row`):** título 13/600 + subtítulo 12 gris + dato mono a la derecha; la fila entera es el área de pulsación cuando abre detalles. Estados: hover (overlay 6 %), foco (aro `accent` 2 px dentro), activa.
- **Punto de estado:** único portador de color. Lleno = sano/Principal; anillo = vence o en transición; coral lleno = falla; gris = caído; discontinuo = suelto.
- **Botón:** secundario por defecto (fondo overlay, sin borde); **primario** relleno `accent-fill` (uno por grupo); **destructivo** texto rojo sin relleno, que en la hoja de confirmación sí lleva relleno rojo. 28 px de alto, radio 6, peso 500/600.
- **Etiqueta de dato (`tag`):** texto mono gris en cápsula neutra; sin color.
- **Control segmentado:** radios nativos con el seleccionado en `group` elevado sobre una pista neutra.
- **Hoja:** group + velo; título 17/600; acciones fijas abajo, primaria a la derecha.
- **Aviso, vacío, cargando:** texto gris, una línea de título, ninguna ilustración.

## Do's and Don'ts

- **Sí:** un único punto de color por fila; filas en grupo; encabezados de grupo en minúscula; puertos en mono; hay un botón primario por grupo, en el acento.
- **No:** tarjetas con fondo o borde de color; mayúsculas con tracking; relleno verde/ámbar/coral en botones; más de un nivel de contenedor; fundidos de entrada; color decorativo; iconos sin significado.
- **Tokens:** cada color vive en `theme.css` con el mismo nombre en ambos temas; `styles.css` no contiene literales; `contrast.test.ts` vigila AA por tema.
