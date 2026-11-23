# Guía de usuario de YouTFast

[Volver a la presentación](README.md) · [Instalar YouTFast](install.md) · [Detalles de plataforma](../../distribution.md)

[English](../../user-guide.md) · [Português (Brasil)](../pt-BR/user-guide.md) · **Español** ·
[Français](../fr/user-guide.md) · [Deutsch](../de/user-guide.md) · [Italiano](../it/user-guide.md)

*Esta es una traducción del [documento original en inglés](../../user-guide.md). Si
difieren, prevalece la versión en inglés.*

## Requisitos

| Qué | Para qué | Arch | Debian/Ubuntu |
|---|---|---|---|
| libmpv 0.42+ | Reproducción, enlazada al compilar | `mpv>=0.42.0` | `libmpv-dev` (comprueba la versión) |
| yt-dlp | Los streams que reproduce mpv y los reintentos de reproducción | `yt-dlp` | `yt-dlp` (mantenlo actualizado) |
| Deno / EJS de yt-dlp correspondiente | Desafíos de extracción de YouTube | `deno` y el paquete EJS de la distribución | Consulta la [configuración de yt-dlp](https://github.com/yt-dlp/yt-dlp/wiki/EJS) |
| Rust 1.98 | Fijado en `rust-toolchain.toml` | `rustup` | `rustup` |
| python-secretstorage | Opcional: iniciar sesión con el login de un navegador basado en Chromium | `python-secretstorage` | `python3-secretstorage` |

mpv 0.42 o posterior debe estar compilado con Lua (los paquetes habituales lo
están): resuelve los enlaces de YouTube mediante su script `ytdl_hook` y espera
hasta que los streams de yt-dlp estén disponibles. Los hooks más antiguos pueden
abrirlos demasiado pronto y recibir un HTTP 403.
Mantén yt-dlp y su runtime JS en el `PATH`. Comprueba la versión del **paquete
libmpv** instalado, no la versión de su API de cliente ni la de un ejecutable mpv
instalado por separado.
Si tu distribución solo ofrece una libmpv más antigua, actualízala desde una fuente
de confianza o usa una versión Flatpak que incluya mpv 0.42+.
La importación desde el navegador también necesita `python3` con el módulo `yt_dlp`;
una instalación de yt-dlp solo como ejecutable no proporciona ese módulo de Python.

## Ejecución

```sh
cargo run --locked --release
cargo run --locked --release -- "https://youtu.be/dQw4w9WgXcQ"   # open a link
cargo run --locked --release -- "rust egui"                      # or search
```

| Tecla | Acción |
|---|---|
| Espacio, K | Reproducir o pausar |
| ←, → | Retroceder o avanzar 5 s |
| J, L | Retroceder o avanzar 10 s |
| Ctrl+← / → (Option en macOS) | Capítulo anterior / siguiente |
| Inicio, Fin; 0–9 | Principio / final; posición en decenas de porcentaje |
| ↑, ↓; M | Desplazar la página verticalmente fuera del reproductor; volumen ±5% dentro del reproductor; silenciar (los Shorts usan ↑/↓ para cambiar de vídeo) |
| C; <, >; coma, punto | Subtítulos; velocidad; fotograma anterior / siguiente en pausa |
| Shift+N, Shift+P | Siguiente / anterior en el origen de reproducción |
| I; P; T; Shift+A | Minirreproductor; ventana de imagen en imagen; modo cine; modo solo audio |
| F, doble clic | Pantalla completa |
| Ctrl/Cmd+K | Paleta de comandos (vistas, acciones del reproductor y de la página, grupos de Configuración, búsquedas recientes e historial reciente) |
| /; ? | Enfocar la búsqueda; abrir la ayuda de atajos |
| Alt+← / →; botones atrás / adelante del ratón | Navegar atrás / adelante (teclas Alt en Linux/Windows) |
| Ctrl/Cmd+R | Actualizar la página actual |
| Esc | Cerrar un menú/la ayuda/una cuenta atrás, salir de pantalla completa o volver atrás |

Los campos de texto y los menús conservan sus propias teclas. Los botones enfocados
se quedan con el Espacio, y un control de volumen enfocado conserva sus teclas de
flecha. Fuera del reproductor, ↑/↓ desplazan verticalmente la página activa y J/K
mueven la selección de elementos (`g`/`G` saltan al principio o al final). Mantén
pulsado el vídeo para reproducirlo temporalmente a 2×; al soltarlo se restablece la
velocidad anterior sin cambiar la velocidad guardada. La velocidad elegida con
`<`/`>` o en el menú **Velocidad de reproducción** del reproductor se recuerda
entre vídeos y reinicios; activa **Recordar para este canal** en ese menú para
guardar una velocidad para el canal actual (la velocidad del canal tiene
prioridad sobre la global, hasta 500 canales, y se puede borrar en
**Configuración → Reproducción → Velocidades de reproducción por canal**). Los
directos en el borde en vivo se mantienen a 1×; si un directo se ha rebobinado
por detrás del borde, los controles de velocidad y la pulsación larga de 2×
siguen disponibles para alcanzar el directo y vuelven a 1× al llegar al borde,
sin sobrescribir las velocidades guardadas. El menú del reproductor también
ofrece repetición, tamaño/fondo de los subtítulos, copiar un enlace con marca de
tiempo y abrir el vídeo en un navegador.

La búsqueda usa directamente la interfaz InnerTube de YouTube, con resultados
separados de vídeos, canales y listas de reproducción. Los filtros de vídeo incluyen
fecha, duración, estado en directo y orden.
Atrás/adelante restaura la página, la consulta, los filtros, la selección y la
posición de desplazamiento. Las tarjetas tienen enlaces al canal, un menú y acciones
rápidas de Ver más tarde/cola. La cola se puede reordenar o recortar; el
minirreproductor es un recuadro de vídeo 16:9 compacto y sin marco en la esquina
inferior derecha (haz clic para volver a la página del vídeo, o pasa el ratón por
encima para ver los controles de reproducir/pausar y cerrar), y la ventana flotante
de imagen en imagen (`P` o el control del reproductor) abre una ventana separada
con controles de reproducir/pausar, cerrar y **Devolver el vídeo a la ventana
principal**. Esto devuelve la reproducción sin pausarla; si la ventana principal
sigue minimizada, restáurala manualmente. En Wayland, la aplicación no puede
mantener esa ventana encima ni traer al frente o restaurar la ventana principal.
Si tu compositor permite reglas de ventanas, usa el ID `youtfast-pip` para
mantenerla encima. La reproducción de listas continúa más allá de la página
cargada inicialmente. El historial del dispositivo permite filtrar y quitar
entradas individuales.

En una ventana ancha, Configuración muestra sus grupos en un índice a su lado: haz
clic en un grupo para desplazarte hasta él. Una fila con un interruptor se puede
pulsar en cualquier parte, no solo sobre el interruptor.

Configuración → Barra lateral elige qué botones de la barra lateral se muestran y en
qué orden (el interruptor los oculta, las flechas los mueven y Restaurar valores
predeterminados lo deshace). Cuenta y Configuración no se pueden ocultar. Vídeos que
me gustan solo aparece con la sesión iniciada; Listas de reproducción está disponible
tanto sin sesión (mostrando listas del dispositivo) como con sesión iniciada.
Las teclas numéricas del 1 al 9 abren los botones en el orden en que los muestra la
barra lateral.

Configuración → Reproducción → Minirreproductor al desplazarse mantiene un vídeo
visible en el minirreproductor de la esquina cuando te desplazas más allá de él para
leer los comentarios. Está desactivado por defecto.
Al volver a desplazarte aparece de nuevo el reproductor integrado; al hacer clic en
el minirreproductor vuelves al vídeo en la parte superior de la página. Cerrar el
minirreproductor pausa la reproducción.

Configuración → Reproducción → **Títulos de la comunidad (DeArrow)** y
**Miniaturas de la comunidad (DeArrow — envía el ID del vídeo)** están
desactivados por defecto. Los títulos de la comunidad consultan `sponsor.ajay.app`
usando solo un prefijo de cuatro dígitos hexadecimales del SHA-256 del ID del
vídeo y muestran el título original de YouTube al pasar el cursor sobre un título
reemplazado. Las miniaturas de la comunidad envían el ID de cada vídeo
directamente a `dearrow-thumb.ajay.app` y vuelven a la miniatura de YouTube cuando
no están disponibles. Todas las respuestas y miniaturas de DeArrow permanecen
solo en una caché limitada en memoria y nunca se guardan en el disco.

Configuración → Reproducción → Búfer de vídeo elige entre la caché estándar del
demuxer de mpv (**Grande — 150 MiB / 50 MiB**, la predeterminada, que almacena varios
minutos por adelantado para conexiones inestables y búsquedas instantáneas) y
**Pequeño — 50 MiB / 20 MiB** (para mantener el uso de RAM en torno a ~200 MB y
gastar menos datos al saltar vídeos pronto).
El modo solo audio (`Shift+A` o el botón de los auriculares en los controles del
reproductor) transmite y decodifica
solo la pista de audio (`ba/b` y `vid=no`) del vídeo actual, y vuelve al vídeo
estándar al abrir otro vídeo o reiniciar la app.

Configuración → Reproducción → **Saltar patrocinios (SponsorBlock)** está
desactivado por defecto y consulta SponsorBlock (`sponsor.ajay.app`) usando solo los
cuatro primeros dígitos hexadecimales del hash SHA-256 del ID del vídeo. Cuando está
activado, elige un comportamiento para cada una de las ocho categorías
(**Patrocinio**, **Autopromoción**, **Recordatorio de interacción**, **Intro**,
**Outro**, **Vista previa**, **Sección sin música**, **Relleno**): **Saltar
automáticamente** (predeterminado para patrocinios, autopromoción y recordatorios de
interacción), **Mostrar botón para saltar** (predeterminado para el resto de
categorías), **Solo marcar en la barra de progreso** o **Ignorar**. Los segmentos de
silenciamiento en categorías configuradas como **Saltar automáticamente** silencian
la reproducción de forma automática dentro del segmento (si no estaba silenciada
antes) y reactivan el sonido al salir, detener, abrir otro vídeo o cambiar el
silencio manualmente.

La página del vídeo también muestra la descripción (las marcas de tiempo saltan a
esa posición), los capítulos y los comentarios, cargados mediante InnerTube. Los
comentarios y las respuestas también muestran adjuntos de imagen y GIF/WebP
animados. Los adjuntos usan la caché acotada de miniaturas, se decodifican en
segundo plano y solo se animan mientras están visibles.
Las páginas de canal tienen pestañas — vídeos,
shorts, en directo, listas de reproducción e información — y una búsqueda entre los
vídeos del propio canal.

Las páginas de canal incluyen un banner y el orden de los vídeos. Las listas de
reproducción muestran su portada, sus metadatos y el elemento que se está
reproduciendo. Los Shorts usan un escenario vertical con controles de
siguiente/anterior y navegación con la rueda. La interfaz empieza en inglés.
Elige English, Português (Brasil), Español, Français, Deutsch o Italiano en
Configuración → Aspecto → Idioma; el cambio se aplica de inmediato y se conserva
entre ejecuciones, también para las notificaciones de escritorio. Las
configuraciones antiguas también usan el inglés por defecto. Los títulos de los
vídeos, los comentarios y las preferencias de subtítulos conservan sus propios
idiomas.

La página del vídeo puede mostrar una transcripción sincronizada con búsqueda y
vistas previas en miniatura en la línea de tiempo cuando YouTube proporciona los
metadatos públicos necesarios. Estas solicitudes opcionales se inician a demanda,
funcionan sin cuenta y usan descargas acotadas. La falta de subtítulos/storyboards o
una restricción de origen pueden dejar la función no disponible; la reproducción
sigue funcionando mediante libmpv.

Para vistas previas sintéticas de la interfaz, compila con `--features demo` y ejecuta:

```sh
cargo run --locked --features demo -- --demo --demo-view watch --demo-size 1280x800
cargo run --locked --features demo -- --demo --demo-view home --demo-light
cargo run --locked --features demo -- --demo --demo-view settings --demo-language pt-BR
cargo run --locked --features demo -- --demo --demo-view theater --demo-maximized
```

El modo de demostración abre una ventana aislada sin cuentas, archivos del usuario,
red, reproducción ni registro multimedia en el escritorio. Muestra estados de
ejemplo, no todas las interacciones con el backend.
`--demo-capture target/example.png` captura esa ventana y la cierra.
`--demo-maximized` inicia la vista previa maximizada. En la vista `watch`,
`--demo-aspect 4:3` (también `16:9` o `21:9`) estira la imagen sintética a otra
forma; `9:16` usa la alternativa vertical. Las vistas `watch`,
`theater` y `fullscreen` muestran esas disposiciones. Los atajos de reproducción siguen
[los atajos publicados por YouTube](https://support.google.com/youtube/answer/7631406),
con las adaptaciones para listas y Shorts descritas arriba.

Las teclas multimedia y los paneles del escritorio (MPRIS, así que `playerctl`
también funciona) controlan la reproducción, y un segundo inicio pasa un enlace o una
búsqueda a la ventana ya abierta en lugar de abrir otra.

## Transmitir a la TV

El selector de TV admite tres vías. Los televisores con YouTube compatibles con
DIAL/Lounge usan red nativa y no requieren ningún programa auxiliar. Esto incluye
modelos Samsung, LG, Roku, Fire TV y Android/Google TV que exponen YouTube mediante
DIAL; cuando una TV ofrece tanto DIAL como Google Cast, el selector conserva ambas
opciones de conexión. Las direcciones IP locales distinguen televisores con el mismo
nombre. Cualquier dispositivo
cuya app de YouTube muestre **Vincular con código de TV** (Apple TV, consolas de
videojuegos, una TV en otra red o tras el aislamiento de una red de invitados) se
puede vincular con ese código. La compatibilidad con Google Cast es opcional y usa
[`catt`](https://github.com/skorokithakis/catt) instalado por separado en el `PATH`.
Por ejemplo, si ya usas pipx, instálalo con `pipx install catt`.
Sigue las instrucciones de instalación de catt para tu sistema operativo, comprueba
`catt --version` en el entorno que inicia YouTFast y luego reinicia YouTFast si
cambiaste el `PATH`. En Windows, YouTFast también encuentra catt donde lo instalan
pip (`pip install --user catt`) y pipx (`pipx install catt`) —
`%APPDATA%\Python\Python3*\Scripts` y `%USERPROFILE%\.local\bin`, carpetas que no
están en el `PATH` que hereda una aplicación de escritorio —, así que basta
cualquiera de los dos comandos, y un YouTFast en ejecución lo encuentra en la
siguiente búsqueda. Mientras falte catt, el selector lo indica como guía junto a
los resultados en lugar de una búsqueda fallida; las rutas por dirección IP y
código de TV no necesitan programa auxiliar. YouTFast nunca instala el programa
auxiliar automáticamente.
El programa auxiliar y sus dependencias de Python no se incluyen en los archivos de
las versiones.
Flatpak no puede usar un programa auxiliar arbitrario del sistema anfitrión; esta
integración aún no está validada para el paquete Flatpak.

1. Enciende la TV o el Chromecast y conéctalo a la red local del ordenador.
   El aislamiento de redes de invitados y el multicast bloqueado pueden impedir la
   detección; la búsqueda se envía por todas las interfaces de red privadas, así que
   las VPN y los puentes de contenedores no ocultan la red de la TV. En Windows, el
   cortafuegos puede descartar las respuestas de los televisores a una búsqueda en
   toda la red: permite YouTFast cuando Windows lo pregunte, o usa las opciones de
   IP o de código de TV indicadas abajo. Esta versión acepta direcciones IP privadas
   o de enlace local. Google Cast usa el puerto 8009; YouTube DIAL usa el endpoint
   anunciado por la TV.
2. Abre un vídeo de YouTube en YouTFast y haz clic en **Transmitir a la TV** junto a
   **Solo audio** en los controles del reproductor, o usa el botón de la barra superior.
   La detección solo se ejecuta cuando abres el selector por primera vez o pulsas
   **Buscar televisores**; no hay ningún escaneo de red en segundo plano.
   Los televisores con YouTube se detectan mediante SSDP y endpoints DIAL verificados;
   también se comprueba si los anuncios de AirPlay y Google Cast admiten YouTube
   DIAL. Si falta una TV, introduce su IP en **Buscar una TV con YouTube por IP** y
   pulsa **Buscar TV**. La búsqueda manual pide a esa dirección su endpoint DIAL y
   luego prueba las rutas habituales en los puertos 8080, 8008 y 8060.
   No se inicia ninguna app durante la detección ni la búsqueda por IP.
   Para vincular con un código, abre YouTube en la TV, luego **Configuración →
   Vincular con código de TV**, escribe el código de 12 dígitos en **Vincular una TV
   con un código** y pulsa **Vincular TV**. El código se usa una vez y no se guarda;
   se llega a la TV vinculada a través de los servidores de YouTube y permanece en la
   lista hasta que cierras YouTFast.
3. Elige el dispositivo. YouTFast envía el enlace canónico del vídeo y la posición
   local redondeada hacia abajo a segundos enteros. Los vídeos en directo piden el
   punto en directo. El reproductor local sigue reproduciendo hasta que la TV
   informa de que el vídeo solicitado se está reproduciendo, y espera hasta un minuto
   por los anuncios y el selector de cuentas de la TV. Con DIAL, elegir la TV se une
   a su app de YouTube si ya está abierta, o la abre, y establece una conexión Lounge
   en memoria; una conexión inactiva se renueva automáticamente. Si la TV te pide
   iniciar sesión, elegir una cuenta o mirar como invitado, elige en la TV; YouTFast
   nunca gestiona ese inicio de sesión.
   Una confirmación tardía nunca pausa otro vídeo local,
   ni siquiera una reproducción recién abierta del mismo vídeo.
4. Usa los controles explícitos de la TV en el panel: reproducir, pausar,
   retroceder/avanzar diez segundos, bajar/subir el volumen diez puntos porcentuales
   y **Detener en la TV**. Los comandos se envían de uno en uno; el panel no
   sincroniza continuamente el estado de la TV.
5. Cerrar el panel solo lo oculta. Vuelve a abrirlo desde la barra superior para
   controlar el mismo destino. **Desconectar y mantener en la TV**, o salir de
   YouTFast, deja la reproducción en la TV y descarta la sesión de control en
   memoria. **Detener en la TV** pide a un receptor Cast que se detenga, o cierra la
   app de YouTube por DIAL; en una TV que no permite cerrarla, o en una vinculada con
   código, se detiene el vídeo. Ninguna de las dos acciones
   reanuda automáticamente el reproductor local.

El panel es un mando a distancia aparte: los controles del reproductor local, los
atajos de teclado y las teclas multimedia del escritorio siguen controlando el
ordenador. El punto del botón de transmisión indica que se conserva un destino, no
que la reproducción siga en curso. Una confirmación inicial correcta es histórica;
no se consultan los cambios hechos en la TV. Puede producirse un tiempo de espera
agotado o un error después de que la TV empiece: comprueba la TV antes de volver a
enviar. El panel conserva un destino incierto para que aún puedas detenerlo o
desconectarte.

Los destinos de Google Cast son Chromecast, Google TV Streamer
y televisores compatibles con Google TV, Android TV, LG, Samsung y otros con Cast
activado. La compatibilidad depende del modelo y el firmware exactos, no de la marca.
Los altavoces de Google conocidos (Home, Nest Mini, Nest Audio, Chromecast Audio)
quedan fuera de la lista; otros dispositivos Cast solo de audio pueden seguir
apareciendo y no son destinos de vídeo. Una TV que no informa de su volumen (por
ejemplo, con el sonido en un receptor HDMI-ARC) mantiene el volumen en su propio mando.
Los destinos DIAL necesitan una app de YouTube que funcione, exponga un
identificador de pantalla y tenga acceso a YouTube Lounge. AirPlay es solo una pista
para la detección; el contenido no se envía mediante AirPlay, Miracast ni DLNA. La
validación nativa en Windows/macOS y en Flatpak está pendiente. La API de Lounge no
está documentada y puede cambiar con independencia de YouTFast; sus referencias de
protocolo son los proyectos ytcast y casttube.

La aplicación de YouTube de la TV y su cuenta gestionan la reproducción, incluidos los
anuncios y las restricciones de acceso. YouTFast no transfiere su sesión de cuenta,
las cookies del navegador ni la biblioteca local. Con **Saltar patrocinios
(SponsorBlock)** activado, YouTFast también comprueba la posición en la TV del vídeo
enviado y envía saltos remotos por DIAL/Lounge
o Google Cast (`catt`). Salta automáticamente los segmentos de salto de las
categorías configuradas como **Saltar automáticamente** (los segmentos de
silenciamiento no se silencian en la TV). Mantén YouTFast
abierto y conectado; desconectar o desactivar SponsorBlock detiene los saltos
automáticos posteriores.
Esto se aplica al vídeo enviado explícitamente, no a los vídeos siguientes que elija
la reproducción automática de la TV. No se saltan los directos, la reproducción en
pausa ni los casos sin telemetría de posición. Las comprobaciones se hacen cada dos
segundos aproximadamente cerca de un segmento y con hasta diez segundos de
separación cuando el siguiente está lejos. La consulta de la posición de la TV y el
arranque del programa auxiliar pueden retrasar un salto; los dos últimos
segundos de un segmento no se tocan. Los eventos de anuncios de Lounge suprimen los
saltos de patrocinios; `catt` no expone un indicador de anuncios separado y fiable,
así que Cast depende del estado de reproducción y de la capacidad de búsqueda del
receptor. Esto no bloquea los anuncios de YouTube.
La validación con TV/Chromecast físicos y en las plataformas nativas sigue pendiente.

La configuración local de subtítulos, la reproducción automática/cola local y las
reglas de sincronización del historial de la cuenta no se transfieren a la TV;
YouTube en la TV puede registrar su propio historial. Los resultados de la detección
y la TV seleccionada se guardan solo en memoria. La salida en bruto y los errores del
programa auxiliar nunca se muestran
ni se registran, ni siquiera con el registro detallado.

## Tu biblioteca, con o sin cuenta

**Inicio** muestra lo más reciente de tus suscripciones — el feed de la cuenta o los
canales RSS del dispositivo — agrupado por días y paginado: desplázate hasta el final
y llega más. Los Shorts están en un estante encima de la lista: los de la cuenta los
marca YouTube, los de los canales del dispositivo vienen de sus pestañas de Shorts.

El **filtrado del feed** en **Inicio** y **Suscripciones** se ejecuta
localmente en este dispositivo: el botón persistente **Ocultar vistos** oculta
los vídeos vistos en Inicio (incluido el estante de Shorts) y de la actividad y
vistas previas de las tarjetas en Suscripciones; los menús de las tarjetas
también ofrecen **No me interesa** (oculta hasta 5.000 vídeos, descartando los
más antiguos) y **No recomendar este canal** (bloquea hasta 1.000 canales,
descartando los más antiguos), excluyéndolos de Inicio, Suscripciones y las
recomendaciones locales. Los resultados de búsqueda y la página propia de un
canal nunca se filtran. Revisa o deshaz vídeos ocultos (**Borrar**) y canales
bloqueados (**Desbloquear**) en **Configuración → Historial**.

**Sin cuenta**, YouTFast guarda las suscripciones, el historial de reproducciones
(con el punto donde se dejó cada vídeo, para reanudarlo), Ver más tarde y las
listas de reproducción locales en este equipo, en `~/.local/share/youtfast/library.json`,
y su configuración (calidad de vídeo, volumen, velocidad de reproducción) en
`~/.config/youtfast/settings.json`. Crea, renombra, elimina y reordena listas del
dispositivo desde **Listas de reproducción** (marcadas como **En este dispositivo**
también con sesión iniciada), y añade o quita vídeos desde el menú **Guardar**
de la página del vídeo o el menú de las tarjetas de vídeo. El feed de inicio viene del RSS
de cada canal suscrito. Trae una biblioteca existente desde
[Google Takeout](https://takeout.google.com): selecciona YouTube y YouTube
Music, con JSON como formato del historial, e importa `subscriptions.csv` y
`watch-history.json`. Nada de esto toca una cuenta de Google. Las importaciones
aceptan archivos de hasta 64 MiB y 20,000 suscripciones únicas, y conservan las
1,000 entradas únicas más recientes del historial. El análisis y la fusión se
ejecutan en segundo plano.
Si la biblioteca cambia durante la fusión, reintenta la importación; esos cambios
más recientes nunca se sustituyen por una instantánea de importación más antigua.

En **Configuración → Historial**, **Respaldar biblioteca** exporta tus
suscripciones del dispositivo, el historial de reproducciones (incluidas las
posiciones de reanudación) y Ver más tarde como un archivo JSON versionado
(`youtfast-library.json`) mediante **Exportar biblioteca (JSON)**, o combina un
respaldo de nuevo mediante **Importar respaldo** con las mismas reglas de fusión
en segundo plano (límite de 64 MiB, hasta 20.000 suscripciones únicas, las 1.000
entradas más recientes del historial y 5.000 vídeos de Ver más tarde). Las
cuentas y sesiones nunca se exportan. **Exportar suscripciones** guarda tus
suscripciones del dispositivo como un archivo OPML 2.0
(`youtfast-subscriptions.opml`) con la URL del feed RSS de cada canal en YouTube
mediante **Exportar OPML**.

**Sincronización entre dispositivos**, en **Configuración → Historial**, conecta
dos de tus ordenadores directamente mediante Tailscale, NetBird o una red local
privada para combinar suscripciones del dispositivo, historial de
reproducciones (incluidas las posiciones de reanudación), Ver más tarde, listas
del dispositivo, vídeos ocultos y canales bloqueados. En un ordenador,
selecciona **Nueva clave**, luego **Copiar clave**, y pégala en **Clave de
emparejamiento** en el otro. Los dispositivos verifican que comparten la clave
y cifran la conexión antes de enviar datos de la biblioteca. Deja vacía
**Dirección del otro dispositivo** y YouTFast encuentra tus otros ordenadores
por sí solo, de tres formas: pregunta en la red local qué dispositivos tienen
la misma clave, pide sus pares a los clientes de línea de comandos `tailscale`
y `netbird`, y los dispositivos que se sincronizaron se cuentan qué otros
ordenadores conocen. Basta con cualquiera de ellas: cuando un ordenador
encuentra a otro, conoce a todos los ordenadores que ese otro conoce, así que
un ordenador nuevo recibe la biblioteca de todos los demás pocos segundos
después de pegar la clave. La búsqueda se repite cada 10 minutos y siempre que
los dispositivos conocidos dejan de responder. En la red local, la pregunta
solo lleva el puerto de escucha y un código que demuestra la clave, nunca la
clave; permite en el cortafuegos la entrada TCP y UDP en el **Puerto de
escucha**. Para usar un solo ordenador, o cuando ninguna búsqueda lo alcanza
(el Flatpak no puede ejecutar los clientes), introduce su dirección (por
ejemplo, `100.64.0.2:38452`). Ajusta **Puerto de escucha** (`1024`–`65535`,
predeterminado `38452`, el mismo en todos los ordenadores) o **Sincronizar
cada** si hace falta. Se rechazan las direcciones IP públicas, y las cuentas de
YouTube, sesiones, marcas de tiempo de alertas y ajustes permanecen en cada
ordenador. Inicia una sincronización inmediata, que también vuelve a buscar
los dispositivos, con **Sincronizar ahora** o **Sincronizar dispositivos
ahora** en la paleta de comandos. Si probaste una versión preliminar de la
sincronización, actualiza ambos ordenadores y genera una clave nueva.

**History sync** (sincronización del historial), en Configuración → Historial, se aplica con la sesión iniciada:

- **Separate** (separado; la opción predeterminada) mantiene separadas las listas de Cuenta y Este dispositivo.
- **Merged in YouTFast** (combinado en YouTFast) abre el Historial en **Todos**, combinando ambas listas sin
  vídeos duplicados. **Cuenta** y **Este dispositivo** siguen disponibles como filtros.
  La búsqueda filtra la lista combinada; las posiciones de reanudación locales y las
  marcas de visto tienen prioridad. No se envía nada a YouTube.
- **Merged and sent to YouTube** (combinado y enviado a YouTube) además informa de los vídeos que realmente
  empiezan a reproducirse aquí, tras dos segundos de reproducción activa, a la cuenta
  activa cuando empieza la reproducción. Los Shorts que se pasan rápidamente no se
  informan. Esto envía informes de reproducción a Google desde tu cuenta; Google
  puede tratarlo como acceso automatizado. Los informes fallidos no interrumpen la
  reproducción. La lista de la cuenta se actualiza la próxima vez que se abre el
  Historial tras un informe correcto.

La lista combinada conserva los encabezados por día de YouTube. Como el historial de
la cuenta tiene etiquetas de día en lugar de horas de reproducción exactas, el orden
entre ambas fuentes es aproximado; las etiquetas de día desconocidas conservan el
orden de la cuenta. El botón de quitar solo está disponible para los vídeos guardados
en este dispositivo y solo elimina esa entrada local.
El historial de la cuenta sigue siendo de solo lectura para las eliminaciones. La
opción no tiene efecto sin la sesión iniciada.

Si un archivo de biblioteca o de configuración no se puede leer, se hace una copia de
seguridad antes de poder guardar los valores predeterminados. Un fallo de lectura o
de copia de seguridad detiene el inicio en lugar de sobrescribir el original. Los
guardados al salir esperan a que terminen los guardados en curso e impiden que
instantáneas más antiguas en cola sustituyan el estado final.

**Con tus cuentas de YouTube**, cada cuenta tiene un nombre y se guarda en el
depósito de claves con su propia entrada: añade varias, cambia de una a otra desde la
página de la cuenta o con `a`, y asocia un inicio de sesión del navegador a un perfil
(`Profile 1`, `Default`, …). El feed, las suscripciones, el historial, Ver más tarde,
los vídeos que me gustan y las listas de reproducción vienen de la cuenta. YouTube no
ofrece inicio de sesión para apps como esta, así que YouTFast usa la sesión que ya
tiene un navegador:

- **El login de tu navegador**: YouTFast lee las cookies de YouTube del navegador
  mediante yt-dlp, y vuelve a leerlas cada vez que caducan.
- **Un archivo cookies.txt**: exportado desde una ventana de incógnito, lo que
  mantiene fuera la propia sesión del navegador.

La sesión vive solo en el depósito de claves del sistema (Secret Service, Keychain,
Credential Manager), nunca en un archivo ni en el registro. **Google puede tratar el
uso de una cuenta de esta forma como acceso automatizado**: se le podría pedir a la
cuenta que se verifique, cerrarle la sesión o suspenderla. Una cuenta secundaria es
la opción segura. Suscribirse y Ver más tarde del dispositivo actúan en local.
Valorar un vídeo o editar una lista de reproducción de la cuenta desde la página del
vídeo modifica esa cuenta seleccionada, y se deshace si YouTube lo rechaza. La
sincronización del historial puede además informar a esa cuenta de las
reproducciones iniciadas cuando eliges explícitamente **Merged and
sent to YouTube**; la opción predeterminada no envía informes de reproducción.

Los cambios de cuenta borran de inmediato las listas privadas y las cachés, y cierran
los menús abiertos. Las escrituras pendientes terminan en su sesión de origen antes
del cambio, y las respuestas tardías no pueden actualizar la interfaz de la nueva
cuenta. Una caducidad de sesión o un fallo de inicio de sesión también descarta las
respuestas privadas del mismo lote pendiente. No se escribe ninguna caché del
cliente en disco; solo la entrada del depósito de claves de la cuenta seleccionada
puede restaurar su sesión.

Los registros usan categorías de diagnóstico fijas en lugar de nombres de cuenta, IDs
de vídeo, perfiles del navegador, URL o texto de errores externos. Esta ocultación
también se aplica a las dependencias y a `--verbose` / `RUST_LOG`; un registro más
detallado no desactiva la protección de la privacidad. Las descargas de miniaturas se
comparten entre la visualización y la caché en disco, con un límite de 4 MiB y cuatro
millones de píxeles cada una, y como máximo tres intentos automáticos ante fallos
transitorios. HTTP 429 respeta
`Retry-After` (segundos o fecha HTTP); los fallos de acceso renuevan la página de
comentarios original una vez por comentario sin adjuntar las cookies de la cuenta a
las descargas de imágenes. Las imágenes eliminadas,
los enlaces sin cambios/caducados, los datos no compatibles y los límites agotados
muestran un control para reintentar. Un elemento de caché dañado se puede sustituir
con independencia de las demás imágenes.
La caché en disco se recorta durante el uso a 2,000 archivos y 128 MiB; las
respuestas `no-store` solo se conservan para la visualización actual.

Los diagnósticos de toda la aplicación se guardan en local en
`$XDG_STATE_HOME/youtfast/logs` (normalmente `~/.local/state/youtfast/logs` en Linux).
`youtfast.log` rota a `youtfast.log.1`; cada uno está limitado a 2 MiB. Estos
archivos cubren toda la aplicación, incluidas las categorías de fallos de descarga de
imágenes y de los decodificadores. No se suben a ningún sitio. En Unix el directorio
solo es accesible para su propietario (0700) y los archivos de registro son 0600. El
mismo filtro de privacidad se ejecuta antes de stderr y de los archivos, también para
los diagnósticos de las dependencias. `--verbose` añade categorías del modo de
animación en caché/en streaming; no hay registros por fotograma. El registro empieza
cuando este binario vuelve a ser dueño de una instancia en ejecución; recompilar no
actualiza un proceso existente. Para seguir el registro actual en Linux (incluida la
rotación):

```sh
tail -F "${XDG_STATE_HOME:-$HOME/.local/state}/youtfast/logs/youtfast.log"
```

En las plataformas sin directorio de estado XDG, los registros usan el subdirectorio
`logs` del directorio de datos locales de aplicación de la plataforma para YouTFast.
Si falla la apertura o la escritura del
archivo, los diagnósticos continúan en stderr.

La reproducción de GIF/WebP en los comentarios usa un hilo decodificador y una
textura reutilizada por cada adjunto visible, con un máximo de dos adjuntos activos.
Los fotogramas se reducen para caber en 320×200 conservando la relación de aspecto.
Las animaciones pequeñas se guardan en caché hasta unos
8 MiB; las más largas continúan de forma secuencial en lugar de retener todos los
fotogramas a tamaño original o rechazar el clip entero al llegar a un umbral de
número de fotogramas. Las reservas controladas de fotogramas, lienzo y texturas
comparten un presupuesto de 64 MiB; la contabilidad del decodificador, las
bibliotecas y el controlador, así como la caché normal de miniaturas, son memoria
adicional del proceso.
Un fotograma que tarda más de 200 ms en decodificarse detiene la decodificación
automática posterior y conserva una vista previa; es un límite de trabajo observado,
no la interrupción de un decodificador nativo en mitad de un fotograma. Las
animaciones fuera de la pantalla se liberan, los controles de pausa detienen las
peticiones de fotogramas, y
se respetan los recuentos de bucles finitos del origen. Los fallos de decodificación
tras un fotograma correcto conservan ese fotograma como vista previa. Bloqueos
externos, contenido eliminado o un origen inutilizable pueden seguir impidiendo una
vista previa o la reproducción.

## Recomendaciones locales

En **Inicio → Para ti**, abre **Ajustes de recomendaciones locales** y pulsa
**Aplicar** para guardar y recargar. Elige la duración, prioriza canales
conocidos o descubre otros nuevos, o da prioridad a vídeos recientes o con
menos visualizaciones. Separa con comas los intereses y los términos
silenciados, con un máximo de 300 caracteres por campo. No se distingue
entre mayúsculas y minúsculas: los intereses
favorecen títulos o nombres de canales coincidentes y los términos
silenciados ocultan los títulos coincidentes. El filtro de duración excluye
los vídeos cuya duración se desconoce.

Con una sesión iniciada, las recomendaciones de YouTube siguen siendo la opción
predeterminada. Activa **Usar la clasificación local en vez de las recomendaciones de YouTube**
para aplicar los ajustes al feed. Si no hay historial apto en el dispositivo,
puedes autorizar el envío de hasta tres intereses a YouTube en búsquedas sin
sesión iniciada. Tras ver el primer vídeo apto, al volver a Inicio, las
recomendaciones locales se actualizan a partir de tu historial. Los términos
silenciados nunca salen del dispositivo.

## Avisos de nuevas subidas

Las notificaciones propias de YouTube no tienen API, así que YouTFast comprueba tus
suscripciones periódicamente (cada 15 minutos por defecto; 5, 15, 30
o 60 en Configuración). La campana de la barra superior abre una lista desplazable
sin salir de la página actual; desde ahí puedes volver a comprobar o borrarla. Abrir
la campana marca los avisos actuales como vistos, pero los mantiene visibles durante
esa visita.
Los vídeos encontrados por una comprobación mientras la lista está abierta aparecen
en ella y también se marcan como vistos.
La entrada Notificaciones de la barra lateral sigue abriendo la página completa. Las
nuevas subidas también pueden aparecer como notificaciones de escritorio. Tanto las
comprobaciones como las notificaciones de escritorio se pueden desactivar en
Configuración; la primera ejecución solo pone en marcha el reloj, para que una
instalación nueva no te inunde de avisos.

## Fuente de la interfaz

Configuración → Aspecto → Fuente de la interfaz. **Sistema** (la predeterminada)
dibuja YouTFast con la fuente de interfaz de tu escritorio: San Francisco en macOS,
Segoe UI en Windows y, en Linux, la fuente que tu escritorio tenga configurada
(la fuente de interfaz de GNOME, o la configuración de GTK o de KDE) o, si no, la que
indique `system-ui` de fontconfig.
**Inter** es la fuente que incluye YouTFast, igual en todos los ordenadores. El
cambio se aplica al instante. Otros sistemas de escritura (árabe, chino, devanagari,
...) se dibujan con las fuentes instaladas en el ordenador en ambos casos.

En Linux, el menú muestra además las fuentes instaladas en el ordenador (las que,
según fontconfig, pueden dibujar texto en inglés): escribe en el campo del menú para
acotar la lista y elige una para dibujar la interfaz con ella. El texto en negrita y
en peso medio usa los pesos propios de la familia cuando los tiene. Si una fuente
elegida se elimina más tarde, YouTFast dibuja con Inter e indica qué fuente falta;
elige otra en el mismo menú. Windows y macOS solo ofrecen Sistema e Inter.

Si la fuente del sistema no se puede encontrar o leer, YouTFast dibuja con Inter y el
grupo Aspecto lo indica. **Informar de esto** abre tu navegador en una nueva
incidencia de GitHub con la versión de YouTFast, tu sistema operativo, la sesión de
escritorio, la fuente que tu escritorio tiene configurada y lo que imprime
`fc-match system-ui` ya escritos; tú la lees,
añades la fuente que usa tu escritorio y decides si enviarla. YouTFast no envía nada
por sí solo. El tiempo del reproductor conserva los dígitos de ancho fijo de Inter
con cualquiera de las dos opciones, para que no baile mientras cuenta.

## Temas

Configuración → Aspecto. **Estilo de la interfaz** permite elegir en directo
entre doce estilos visuales: **Cupertino Quiet** (predeterminado), **Liquid
Glass**, **Cinema Minimal**, **Warm Studio**, **Adwaita (GNOME)**, **Fluent
Mica**, **Industrial (Braun)**, **Technical (Tiling)**, **Tactile Modern**,
**Material Tonal**, **Paper** y **Clásico** (la apariencia y el ritmo
originales). Los temas oscuro y claro integrados adaptan sus colores al estilo
elegido, mientras que las paletas personalizadas y los temas de Omarchy
conservan sus propios colores y adoptan las superficies, elevación tonal,
esquinas, bordes, barra flotante del reproductor y transiciones del estilo.

**Automático** sigue en directo el tema del escritorio
Omarchy en Linux, y es oscuro en los demás casos. **Oscuro** y **Claro** son los
colores propios de YouTFast. Las ocho paletas compartidas (Catppuccin, Nord, Rosé
Pine, Tokyo Night y más) se instalan en `~/.config/youtfast/themes` en la primera
ejecución como archivos JSON sencillos que puedes editar, copiar o eliminar; añade
los tuyos ahí con el mismo formato (una `base` más colores con nombre, consulta
[fastframe-theme](https://github.com/crmne/fastframe)). Una compilación desde el
código fuente no activa el hook de Omarchy (`contrib/omarchy/youtfast-theme`). Los
paquetes nativos de Linux lo incluyen para instalarlo opcionalmente. Flatpak no
activa hooks del sistema anfitrión ni lee implícitamente sus temas. El seguimiento
nativo del tema también funciona sin el hook cuando los archivos del tema son
accesibles.

## Información y actualizaciones

Configuración → Información muestra la versión instalada de YouTFast, la licencia y el
registro de cambios incluido, y busca en GitHub Releases versiones estables más
nuevas al iniciar y cada 12 horas mientras esté abierto:

- **Notificar sobre nuevas versiones** (activado por defecto) envía una notificación
  de escritorio del sistema cuando se publica una nueva versión y resalta
  Configuración en la barra lateral, con acciones rápidas en el aviso de la
  notificación para actualizar, abrir Información o **Reiniciar ahora** cuando
  esté lista.
- **Actualizaciones automáticas (minor y patch)** (desactivado por defecto) descarga,
  verifica (SHA-256 frente al `checksums.txt` de la versión) e instala automáticamente
  en segundo plano las actualizaciones minor (`0.x.0`) y patch (`0.0.x`) para
  instalaciones modificables por el usuario (Linux nativo, Windows instalador/portátil,
  macOS `.app`), y luego muestra el registro de lo que cambió en Configuración →
  Información.
- **Ajuste al método de instalación**: las instalaciones `pacman` de Arch se
  actualizan mediante `pkexec pacman -U` al hacer clic (nunca sin supervisión en
  segundo plano), las instalaciones de Windows (instalador/portátil) preparan y
  aplican la actualización limpiamente al reiniciar o salir para no dejar DLL
  cargadas incompatibles, y las instalaciones Flatpak en entorno aislado
  notifican y ofrecen copiar el comando con un clic en Configuración → Información.
- **Las versiones mayores (`x.0.0`)** nunca se instalan automáticamente; requieren
  pulsar el botón **Actualizar** en la app para confirmar e instalar la versión.

## Ayuda

YouTube rechaza los streams de un vídeo durante unos segundos después de
entregarlos (hasta su hora `available_at`). libmpv 0.42+ (con Lua) espera esa
hora; con una libmpv anterior, YouTFast vuelve a intentar abrir el stream
rechazado tras 0, 1, 3 y 7 segundos, así que un vídeo puede empezar unos
segundos más tarde que con la 0.42. Actualizar libmpv a 0.42+, o usar el
Flatpak, espera el stream directamente en lugar de reintentar.

Si la reproducción muestra **"No se pudo reproducir este vídeo. El reproductor
no identificó la causa. Intenta volver a abrir el vídeo."**, el stream siguió
rechazado tras unos 11 segundos de intentos, o falló otra cosa: sigue los
[pasos de solución de problemas de la instalación](install.md#si-algo-sale-mal).

Lee [CONTRIBUTING.md](../../../CONTRIBUTING.md) para informar de errores y
[SECURITY.md](../../../SECURITY.md) para problemas privados de cuentas o de seguridad.
