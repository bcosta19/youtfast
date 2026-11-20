# Guida utente di YouTFast

[Torna alla panoramica](README.md) · [Installare YouTFast](install.md) · [Dettagli delle piattaforme](../../distribution.md)

[English](../../user-guide.md) · [Português (Brasil)](../pt-BR/user-guide.md) · [Español](../es/user-guide.md) ·
[Français](../fr/user-guide.md) · [Deutsch](../de/user-guide.md) · **Italiano**

*Questa è una traduzione del [documento originale in inglese](../../user-guide.md); in caso di differenze, prevale la versione inglese.*

## Requisiti

| Cosa | Perché | Arch | Debian/Ubuntu |
|---|---|---|---|
| libmpv 0.42+ | Riproduzione, collegata in fase di compilazione | `mpv>=0.42.0` | `libmpv-dev` (verifica la versione) |
| yt-dlp | I flussi riprodotti da mpv e i nuovi tentativi di riproduzione | `yt-dlp` | `yt-dlp` (mantienilo aggiornato) |
| Deno / yt-dlp EJS corrispondente | Sfide di estrazione di YouTube | `deno` e il pacchetto EJS della distribuzione | Vedi la [configurazione di yt-dlp](https://github.com/yt-dlp/yt-dlp/wiki/EJS) |
| Rust 1.98 | Fissato in `rust-toolchain.toml` | `rustup` | `rustup` |
| python-secretstorage | Facoltativo: accesso con il login di un browser basato su Chromium | `python-secretstorage` | `python3-secretstorage` |

mpv 0.42 o successiva deve essere compilata con Lua (i pacchetti usuali lo sono):
risolve i link di YouTube tramite il suo script `ytdl_hook` e attende che i flussi
di yt-dlp diventino disponibili. Gli hook più vecchi possono aprirli troppo presto
e ricevere un HTTP 403. Mantieni yt-dlp e il suo runtime JS nel `PATH`. Controlla
la versione del **pacchetto libmpv** installato, non la versione della sua API
client né quella di un eseguibile mpv installato separatamente.
Se la tua distribuzione fornisce solo una libmpv più vecchia, aggiornala da una
fonte affidabile o usa una release Flatpak che contenga mpv 0.42+.
L'importazione dal browser richiede anche `python3` con il modulo `yt_dlp`; un'installazione
di yt-dlp solo come eseguibile non fornisce quel modulo Python.

## Avvio

```sh
cargo run --locked --release
cargo run --locked --release -- "https://youtu.be/dQw4w9WgXcQ"   # open a link
cargo run --locked --release -- "rust egui"                      # or search
```

| Tasto | Azione |
|---|---|
| Spazio, K | Riproduci o metti in pausa |
| ←, → | Indietro o avanti di 5 s |
| J, L | Indietro o avanti di 10 s |
| Ctrl+← / → (Option su macOS) | Capitolo precedente / successivo |
| Home, Fine; 0–9 | Inizio / fine; posizione in decine di percentuale |
| ↑, ↓; M | Scorrimento verticale della pagina fuori dal player; volume ±5% nel player; disattiva l'audio (negli Shorts ↑/↓ cambiano video) |
| C; <, >; virgola, punto | Sottotitoli; velocità; fotogramma precedente / successivo in pausa |
| Shift+N, Shift+P | Successivo / precedente nella sorgente di riproduzione |
| I; P; T; Shift+A | Miniplayer; finestra picture-in-picture; modalità cinema; modalità Solo audio |
| F, doppio clic | Schermo intero |
| /; ? | Attiva la ricerca; apri l'aiuto delle scorciatoie |
| Alt+← / →; pulsanti indietro / avanti del mouse | Naviga indietro / avanti (tasti Alt su Linux/Windows) |
| Ctrl/Cmd+R | Aggiorna la pagina corrente |
| Esc | Chiudi un menu/l'aiuto/un conto alla rovescia, esci dallo schermo intero o torna indietro |

I campi di testo e i menu mantengono i propri tasti. I pulsanti con il focus
gestiscono lo Spazio e un cursore del volume con il focus mantiene i tasti freccia.
Fuori dal player, ↑/↓ scorrono verticalmente la pagina attiva e J/K spostano la
selezione degli elementi (`g`/`G` saltano all'inizio o alla fine). Tieni premuto
sul video per riprodurlo temporaneamente a 2×; al rilascio torna la velocità
precedente senza modificare la velocità salvata. La velocità scelta tramite
`<`/`>` o nel menu **Velocità di riproduzione** del player viene ricordata tra
un video e l'altro e dopo il riavvio; attiva **Ricorda per questo canale** in
quel menu per salvare una velocità per il canale attuale (la velocità del canale
prevale su quella globale, fino a 500 canali, e può essere cancellata in
**Impostazioni → Riproduzione → Velocità di riproduzione per canale**). Le
dirette restano a 1× al bordo dal vivo; se una diretta è indietro rispetto al
bordo, i controlli della velocità e la pressione prolungata 2× restano
disponibili per recuperare e tornano a 1× raggiunto il bordo, senza sovrascrivere
le velocità salvate. Il menu del player offre anche la ripetizione,
dimensione/sfondo dei sottotitoli, la copia di un link con il minutaggio e
l'apertura del video nel browser.

La ricerca usa direttamente l'interfaccia InnerTube di YouTube, con risultati
separati per video, canali e playlist. I filtri dei video includono data, durata,
stato della diretta e ordinamento.
Indietro/avanti ripristina pagina, query, filtri, selezione e posizione di scorrimento.
Le schede hanno link al canale, un menu e azioni rapide per Guarda più tardi e per
la coda. La coda si può riordinare o accorciare; il miniplayer è un riquadro video
compatto 16:9 senza bordi nell'angolo in basso a destra (fai clic per tornare alla
pagina del video, o passaci sopra con il mouse per i controlli di riproduzione/pausa
e chiusura), mentre la finestra mobile picture-in-picture (`P` o il controllo del
player) apre una finestra di sistema separata, senza bordi e sempre in primo
piano, con controlli di riproduzione/pausa, chiusura e ritorno all'app. La riproduzione delle playlist continua oltre la pagina caricata
inizialmente. La cronologia del dispositivo permette di filtrare e rimuovere singole voci.

In una finestra larga, Impostazioni elenca i suoi gruppi in un indice accanto a
essi: fai clic su un gruppo per scorrere fino a esso. Una riga con un interruttore
si può cliccare in qualsiasi punto, non solo sull'interruttore.

Impostazioni → Barra laterale sceglie quali pulsanti della barra laterale mostrare e
in che ordine (l'interruttore per nascondere, le frecce per spostare, Ripristina
predefiniti per annullare). Account e Impostazioni non si possono nascondere. Video
a cui hai messo Mi piace compare comunque solo dopo l'accesso; Playlist è disponibile
sia da disconnesso (mostrando le playlist del dispositivo) sia dopo l'accesso.
I tasti numerici da 1 a 9 aprono i pulsanti nell'ordine mostrato dalla barra laterale.

Impostazioni → Riproduzione → Miniplayer durante lo scorrimento mantiene un video
visibile nel miniplayer d'angolo quando scorri oltre per leggere i commenti. È
disattivato per impostazione predefinita. Tornando su riappare il player nella
pagina; un clic sul miniplayer riporta al video in cima alla pagina. Chiudere il
miniplayer mette in pausa la riproduzione.

Impostazioni → Riproduzione → **Titoli della community (DeArrow)** e **Miniature
della community (DeArrow — invia l'ID del video)** sono disattivati per
impostazione predefinita. I titoli della community interrogano `sponsor.ajay.app`
usando solo un prefisso di quattro cifre esadecimali dello SHA-256 dell'ID del
video e mostrano il titolo originale di YouTube passando il puntatore su un
titolo sostituito. Le miniature della community inviano l'ID di ciascun video
direttamente a `dearrow-thumb.ajay.app` e tornano alla miniatura di YouTube se
non disponibili. Tutte le risposte e le miniature DeArrow restano solo in una
cache limitata in memoria e non vengono mai salvate su disco.

Impostazioni → Riproduzione → Buffer video sceglie tra la cache standard del demuxer
di mpv (**Grande — 150 MiB / 50 MiB**, il valore predefinito, che bufferizza diversi
minuti in anticipo per le connessioni instabili e le ricerche istantanee) e
**Piccolo — 50 MiB / 20 MiB** (per mantenere l'uso della RAM intorno a ~200 MB e
consumare meno dati quando salti presto i video).
La modalità Solo audio (`Shift+A` o il pulsante delle cuffie nei controlli del player)
scarica e decodifica
solo la traccia audio (`ba/b` e `vid=no`) del video corrente e torna al video
standard quando apri un altro video o riavvii l'app.

Impostazioni → Riproduzione → **Salta sponsor (SponsorBlock)** è disattivato per
impostazione predefinita e interroga SponsorBlock (`sponsor.ajay.app`) usando solo le
prime quattro cifre esadecimali dell'hash SHA-256 dell'ID del video. Quando è
attivo, scegli un comportamento per ciascuna delle otto categorie (**Sponsor**,
**Autopromozione**, **Promemoria di interazione**, **Intro**, **Outro**,
**Anteprima**, **Sezione non musicale**, **Riempitivo**): **Salta automaticamente**
(predefinito per sponsor, autopromozione e promemoria di interazione), **Mostra
pulsante per saltare** (predefinito per le altre categorie), **Segna solo sulla
barra di avanzamento** o **Ignora**. I segmenti di silenziamento nelle categorie
impostate su **Salta automaticamente** silenziano automaticamente la riproduzione
all'interno del segmento (se non era già silenziata) e riattivano l'audio all'uscita,
all'arresto, all'apertura di un altro video o al cambio manuale dell'audio.

La pagina del video mostra anche la descrizione (i minutaggi spostano la
riproduzione), i capitoli e i commenti, caricati tramite InnerTube. Commenti e
risposte mostrano anche gli allegati di immagini e di GIF/WebP animate. Gli allegati
usano la cache limitata delle miniature, vengono decodificati in background e si
animano solo quando sono visibili.
Le pagine dei canali hanno delle schede — video,
shorts, dirette, playlist e informazioni — e una ricerca tra i video del
canale stesso.

Le pagine dei canali includono un banner e l'ordinamento dei video. Le playlist
mostrano la copertina, i metadati e l'elemento in riproduzione. Gli Shorts usano
uno spazio verticale con controlli successivo/precedente e navigazione con la
rotella. L'interfaccia parte in inglese.
Scegli English, Português (Brasil), Español, Français, Deutsch o Italiano in
Impostazioni → Aspetto → Lingua; la modifica si applica subito e viene mantenuta
tra un avvio e l'altro, anche per le notifiche desktop. Anche le impostazioni più
vecchie partono in inglese. Titoli dei video, commenti e preferenze dei sottotitoli
mantengono le proprie lingue.

La pagina del video può mostrare una trascrizione sincronizzata e ricercabile e le
anteprime delle miniature sulla barra di avanzamento quando YouTube fornisce i
metadati pubblici necessari. Queste richieste facoltative partono su richiesta,
funzionano senza account e usano download limitati. Sottotitoli/storyboard
mancanti o una restrizione a monte possono rendere la funzione non disponibile; la
riproduzione resta affidata a libmpv.

Per anteprime sintetiche dell'interfaccia, compila con `--features demo` ed esegui:

```sh
cargo run --locked --features demo -- --demo --demo-view watch --demo-size 1280x800
cargo run --locked --features demo -- --demo --demo-view home --demo-light
cargo run --locked --features demo -- --demo --demo-view settings --demo-language pt-BR
cargo run --locked --features demo -- --demo --demo-view theater --demo-maximized
```

La modalità demo apre una finestra isolata senza account, file dell'utente, rete,
riproduzione o registrazione multimediale nel desktop. Mostra stati predefiniti,
non ogni interazione con il backend. `--demo-capture target/example.png` cattura
quella finestra e la chiude. `--demo-maximized` avvia l'anteprima a finestra
massimizzata. Nella vista del video, `--demo-aspect 4:3` (anche `16:9` o `21:9`)
adatta l'immagine sintetica a un'altra proporzione; `9:16` usa la variante
verticale. Le viste `watch`, `theater` e `fullscreen` mostrano quei layout. I tasti
di riproduzione seguono le
[scorciatoie pubblicate da YouTube](https://support.google.com/youtube/answer/7631406),
con gli adattamenti per gli elenchi e gli Shorts descritti sopra.

I tasti multimediali e i pannelli del desktop (MPRIS, quindi funziona anche `playerctl`)
controllano la riproduzione, e un secondo avvio passa un link o una ricerca alla
finestra già aperta invece di aprirne un'altra.

## Trasmetti alla TV

La scelta della TV supporta tre percorsi. Le TV con YouTube tramite DIAL/Lounge
usano la rete in modo nativo e non richiedono alcun programma di supporto. Ciò
include i modelli Samsung, LG, Roku, Fire TV e Android/Google TV che espongono
YouTube tramite DIAL; quando una TV offre sia DIAL sia Google Cast, l'elenco
mantiene entrambe le opzioni di connessione. Gli indirizzi IP locali distinguono
le TV con lo stesso nome. Qualsiasi dispositivo
la cui app YouTube mostri **Collega con codice TV** (Apple TV, console di gioco,
una TV su un'altra rete o dietro l'isolamento della rete ospiti) può invece essere
collegato con quel codice. Il supporto a Google Cast è facoltativo e usa
[`catt`](https://github.com/skorokithakis/catt) installato separatamente nel `PATH`.
Per esempio, se usi già pipx, installalo con `pipx install catt`.
Segui le istruzioni di installazione di catt per il tuo sistema operativo,
verifica `catt --version` nell'ambiente che avvia YouTFast, poi riavvia YouTFast
se hai modificato il `PATH`. YouTFast non installa mai automaticamente il programma
di supporto. Il programma di supporto e le sue dipendenze Python non sono inclusi
negli archivi delle release.
Flatpak non può usare un programma di supporto arbitrario dell'host; questa
integrazione non è ancora qualificata per il pacchetto Flatpak.

1. Accendi la TV o il Chromecast e collegalo alla rete locale del computer.
   L'isolamento della rete ospiti e il multicast bloccato possono impedire il
   rilevamento; la ricerca viene inviata su ogni interfaccia di rete privata,
   quindi VPN e bridge dei container non nascondono la rete della TV. Su Windows,
   il firewall può scartare le risposte delle TV a una ricerca sull'intera rete:
   consenti YouTFast quando Windows lo chiede, oppure usa le opzioni con IP o con
   codice TV descritte sotto. Questa versione accetta indirizzi IP privati/link-local.
   Google Cast usa la porta 8009; YouTube DIAL usa l'endpoint annunciato dalla TV.
2. Apri un video di YouTube in YouTFast e fai clic su **Trasmetti alla TV** accanto
   a **Solo audio** nei controlli del player, oppure usa il pulsante nella barra superiore.
   Il rilevamento parte solo quando apri la scelta della TV per la prima volta o
   premi **Cerca TV**; non c'è alcuna scansione della rete in background.
   Le TV con YouTube vengono rilevate tramite SSDP ed endpoint DIAL verificati;
   anche gli annunci AirPlay e Google Cast vengono controllati per il supporto a
   YouTube DIAL. Se una TV manca, inserisci il suo IP in **Cerca una TV YouTube
   tramite IP** e premi **Cerca TV**. La ricerca manuale chiede a quell'indirizzo il
   suo endpoint DIAL, poi prova i percorsi comuni sulle porte 8080, 8008 e 8060.
   Nessuna app viene avviata durante il rilevamento o la ricerca per IP.
   Per collegare con un codice, apri YouTube sulla TV, poi **Impostazioni → Collega
   con codice TV**, digita il codice di 12 cifre in **Collega una TV con un codice**
   e premi **Collega TV**. Il codice viene usato una sola volta e non viene
   conservato; la TV collegata viene raggiunta tramite i server di YouTube e resta
   nell'elenco finché YouTFast non viene chiuso.
3. Scegli il dispositivo. YouTFast invia il link canonico del video e la posizione
   locale arrotondata per difetto al secondo intero. I video in diretta richiedono
   il punto live. Il player locale continua a riprodurre finché la TV non segnala
   il video richiesto in riproduzione, attendendo fino a un minuto per gli annunci
   e per la scelta dell'account sulla TV. Con DIAL, scegliere la TV si collega alla
   sua app YouTube se è già aperta, oppure la apre, e stabilisce una connessione
   Lounge in memoria; una connessione inattiva viene rinnovata automaticamente. Se
   la TV ti chiede di accedere, di scegliere un account o di guardare come ospite,
   scegli sulla TV; YouTFast non gestisce mai quell'accesso.
   Una conferma tardiva non mette mai in pausa un altro video
   locale, nemmeno una nuova riproduzione dello stesso video.
4. Usa i controlli espliciti della TV nel pannello: riproduci, pausa, indietro/avanti
   di dieci secondi, volume giù/su di dieci punti percentuali e **Interrompi sulla TV**.
   I comandi vengono inviati uno alla volta; il pannello non sincronizza
   continuamente lo stato della TV.
5. Chiudere il pannello lo nasconde soltanto. Riaprilo dalla barra superiore per
   controllare la stessa destinazione. **Disconnetti senza fermare la TV**, o la
   chiusura di YouTFast, lascia la riproduzione sulla TV e scarta la sessione di
   controllo in memoria. **Interrompi sulla TV** chiede a un ricevitore Cast di
   fermarsi, oppure chiude l'app YouTube DIAL; una TV che non ne permette la
   chiusura, o una collegata con un codice, ferma il video. Nessuna delle due azioni
   riprende automaticamente il player locale.

Il pannello è un telecomando separato: i controlli del player locale, le scorciatoie
da tastiera e i tasti multimediali del desktop continuano a controllare il computer.
Il punto sul pulsante Cast indica che una destinazione è mantenuta, non che la
riproduzione sia ancora in corso. Una conferma iniziale riuscita riflette un
momento passato; i cambiamenti lato TV non vengono interrogati. Un timeout
o un errore può verificarsi dopo l'avvio sulla TV: controlla la TV prima di
inviare di nuovo. Il pannello mantiene una destinazione incerta, così puoi
comunque fermarla o disconnetterti.

Le destinazioni Google Cast sono Chromecast, Google TV Streamer
e le TV Google TV, Android TV, LG, Samsung compatibili e altre TV con Cast
attivo. Il supporto dipende dal modello e dal firmware esatti, non dalla marca.
Gli altoparlanti Google noti (Home, Nest Mini, Nest Audio, Chromecast Audio) sono
esclusi dall'elenco; altri dispositivi Cast solo audio possono comunque comparire e
non sono destinazioni video. Una TV che non comunica il volume (per esempio, con
l'audio su un ricevitore HDMI-ARC) mantiene il volume sul proprio telecomando.
Le destinazioni DIAL richiedono un'app YouTube funzionante che esponga un
identificatore dello schermo e l'accesso a YouTube Lounge. AirPlay è solo un
indizio per il rilevamento; i contenuti non vengono inviati tramite AirPlay,
Miracast o DLNA. La qualificazione nativa su Windows/macOS e quella di Flatpak sono
in sospeso. L'API Lounge non è documentata e può cambiare indipendentemente da
YouTFast; i riferimenti per il suo protocollo sono i progetti ytcast e casttube.

L'applicazione YouTube e l'account della TV gestiscono la riproduzione, compresi
annunci e restrizioni di accesso. YouTFast non trasferisce la propria sessione
dell'account, i cookie del browser né la raccolta locale. Con **Salta sponsor
(SponsorBlock)** attivo, YouTFast controlla anche la posizione sulla TV del video
inviato e invia spostamenti remoti tramite DIAL/Lounge o Google Cast (`catt`). Salta
automaticamente i segmenti di salto per le categorie impostate su **Salta
automaticamente** (i segmenti di silenziamento non vengono silenziati sulla TV). Mantieni YouTFast in esecuzione e
connesso; disconnettersi o disattivare SponsorBlock ferma ulteriori spostamenti
automatici. Questo vale per il video inviato esplicitamente, non per i video
successivi scelti dalla riproduzione automatica della TV. Dirette, riproduzione in
pausa e assenza di telemetria sulla posizione non vengono saltate. I controlli
avvengono circa ogni due secondi vicino a un segmento e fino a dieci secondi di
distanza quando il successivo è lontano. L'interrogazione della posizione sulla TV
e l'avvio del programma di supporto possono ritardare un salto; gli ultimi due
secondi di un segmento non vengono toccati. Gli eventi pubblicitari di Lounge
sopprimono gli spostamenti per gli sponsor; `catt` non espone un indicatore
separato e affidabile per gli annunci, quindi Cast si basa sullo stato di
riproduzione e sulla capacità di spostamento del ricevitore. Questo non blocca gli
annunci di YouTube. La qualificazione su TV/Chromecast fisici e sulle piattaforme
native resta in sospeso.

Le impostazioni locali dei sottotitoli, la riproduzione automatica/coda locale e le
regole di sincronizzazione della cronologia dell'account non vengono trasferite
alla TV;
YouTube sulla TV può registrare la propria cronologia. I risultati del rilevamento
e la TV selezionata sono mantenuti solo in memoria. L'output grezzo e gli errori
del programma di supporto non vengono mai mostrati né registrati nei log, nemmeno
con il logging dettagliato.

## La tua raccolta, con o senza account

**Home** mostra le novità più recenti delle tue iscrizioni — il feed dell'account o
i canali RSS del dispositivo — raggruppate per giorno e paginate: scorri fino in
fondo e ne arrivano altre. Gli Shorts stanno su uno scaffale sopra l'elenco: quelli
dell'account sono indicati da YouTube, quelli dei canali del dispositivo provengono
dalle loro schede Shorts.

Il **filtro del feed** in **Home** e **Iscrizioni** viene eseguito localmente su
questo dispositivo: il pulsante persistente **Nascondi visti** nasconde i video
visti nella Home (incluso lo scaffale Shorts) e dall'attività e dalle anteprime
delle schede in Iscrizioni; i menu delle schede offrono anche **Non mi
interessa** (nasconde fino a 5.000 video, rimuovendo i più vecchi) e **Non
consigliare il canale** (blocca fino a 1.000 canali, rimuovendo i più vecchi),
escludendoli da Home, Iscrizioni e consigli locali. I risultati di ricerca e la
pagina del singolo canale non vengono mai filtrati. Controlla o annulla i video
nascosti (**Cancella**) e i canali bloccati (**Sblocca**) in **Impostazioni →
Cronologia**.

**Senza un account**, YouTFast conserva iscrizioni, cronologia delle visualizzazioni
(con il punto in cui hai lasciato ogni video, per riprenderlo), Guarda più tardi e
le playlist locali su questo computer, in `~/.local/share/youtfast/library.json`, e
le sue impostazioni (qualità video, volume, velocità di riproduzione) in
`~/.config/youtfast/settings.json`. Crea, rinomina, elimina e riordina le playlist
del dispositivo da **Playlist** (contrassegnate come **Su questo dispositivo** anche
quando hai effettuato l'accesso) e aggiungi o rimuovi video dal menu **Salva**
della pagina di riproduzione o dal menu delle schede video. Il feed della Home
proviene dall'RSS di ogni canale seguito. Porta qui una raccolta esistente da
[Google Takeout](https://takeout.google.com): scegli YouTube e YouTube Music, con
JSON come formato della cronologia, e importa `subscriptions.csv` e
`watch-history.json`. Niente di tutto questo tocca un account Google. Le
importazioni accettano file fino a 64 MiB e 20.000 iscrizioni uniche, e conservano
le 1.000 voci di cronologia uniche più recenti. Analisi e unione avvengono in
background. Se la raccolta cambia durante l'unione, ripeti l'importazione; quelle
modifiche più recenti non vengono mai sostituite da un'istantanea d'importazione
più vecchia.

In **Impostazioni → Cronologia**, **Backup della raccolta** esporta le iscrizioni
del dispositivo, la cronologia delle visualizzazioni (comprese le posizioni di
ripresa) e Guarda più tardi come file JSON versionato (`youtfast-library.json`)
tramite **Esporta raccolta (JSON)**, oppure unisce un backup tramite **Importa
backup** con le stesse regole di unione in background (limite di 64 MiB, fino a
20.000 iscrizioni uniche, le 1.000 voci di cronologia più recenti e 5.000 video
di Guarda più tardi). Account e sessioni non vengono mai esportati. **Esporta
iscrizioni** salva le iscrizioni del dispositivo come file OPML 2.0
(`youtfast-subscriptions.opml`) con l'URL del feed RSS YouTube di ogni canale
tramite **Esporta OPML**.

**History sync** (sincronizzazione della cronologia) in Impostazioni → Cronologia si
applica quando hai effettuato l'accesso:

- **Separate** (separata, il valore predefinito) mantiene separati gli elenchi
  Account e Questo dispositivo.
- **Merged in YouTFast** (unita in YouTFast) apre Cronologia su **Tutti**, combinando
  entrambi gli elenchi senza video duplicati. **Account** e **Questo dispositivo**
  restano disponibili come filtri. La ricerca filtra l'elenco combinato; le posizioni
  di ripresa locali e le note di visione hanno la precedenza. Niente viene inviato a YouTube.
- **Merged and sent to YouTube** (unita e inviata a YouTube) segnala inoltre i video
  che iniziano davvero a essere riprodotti qui, dopo due secondi di riproduzione
  attiva, all'account attivo all'inizio della riproduzione. Gli Shorts scorsi
  rapidamente non vengono segnalati. Questo invia a Google rapporti di riproduzione
  dal tuo account; Google può considerarlo un accesso automatizzato. Le segnalazioni
  non riuscite non interrompono la riproduzione. L'elenco dell'account si aggiorna
  alla successiva apertura di Cronologia dopo una segnalazione riuscita.

L'elenco unito mantiene le intestazioni per giorno di YouTube. Poiché la cronologia
dell'account ha etichette per giorno anziché orari di visione esatti, l'ordine tra
le due fonti è un'approssimazione; le etichette di giorno non riconosciute mantengono
l'ordine dell'account. Il pulsante di rimozione è disponibile solo per i video
conservati su questo dispositivo ed elimina solo quella voce locale. La cronologia
dell'account resta in sola lettura per l'eliminazione. L'impostazione non ha effetto
senza accesso.

Un file della raccolta o delle impostazioni illeggibile viene copiato come backup
prima che si possano salvare i valori predefiniti. Un errore di lettura o di backup
interrompe l'avvio invece di sovrascrivere l'originale. I salvataggi alla chiusura
attendono quelli in corso e impediscono che istantanee più vecchie in coda
sostituiscano lo stato finale.

**Con i tuoi account YouTube**, ogni account ha un nome ed è conservato nel
portachiavi in una voce propria: aggiungine diversi, passa dall'uno all'altro dalla
pagina dell'account o con `a`, e indirizza un accesso tramite browser a un profilo
(`Profile 1`, `Default`, …). Feed, iscrizioni, cronologia, Guarda più tardi, video
a cui hai messo Mi piace e playlist provengono dall'account. YouTube non offre un
accesso per app come questa, quindi YouTFast usa la sessione che un browser ha già:

- **Il login del tuo browser**: YouTFast legge i cookie di YouTube del browser
  tramite yt-dlp, e li rilegge ogni volta che scadono.
- **Un file cookies.txt**: esportato da una finestra anonima, il che tiene fuori la
  sessione del browser stesso.

La sessione vive solo nel portachiavi di sistema (Secret Service, Portachiavi,
Credential Manager), mai in un file o nel log. **Usare un account in questo modo
può essere considerato da Google un accesso automatizzato**: all'account potrebbe
essere chiesto di verificarsi, potrebbe essere disconnesso o sospeso. Un account
secondario è la scelta sicura. L'iscrizione e il Guarda più tardi del dispositivo
agiscono in locale. Valutare un video o modificare una playlist dell'account dalla
pagina del video modifica l'account selezionato, con annullamento se YouTube rifiuta
l'operazione. La sincronizzazione della cronologia può inoltre segnalare a
quell'account le riproduzioni avviate quando scegli esplicitamente **Merged and
sent to YouTube**; il valore predefinito non invia rapporti di riproduzione.

I cambi di account svuotano subito gli elenchi privati e le cache e chiudono i menu
aperti. Le scritture in sospeso terminano nella sessione da cui sono partite prima
del cambio, e le risposte tardive non possono aggiornare l'interfaccia del nuovo
account. Una sessione scaduta o un accesso non riuscito scartano anche le risposte
private dello stesso lotto in sospeso. Nessuna cache del client viene scritta su
disco; solo la voce nel portachiavi dell'account selezionato può ripristinarne la
sessione.

I log usano categorie diagnostiche fisse anziché nomi di account, ID di video,
profili del browser, URL o testi di errore esterni. Questo oscuramento si applica
anche alle dipendenze e a `--verbose` / `RUST_LOG`; un logging più dettagliato non
disattiva la protezione della privacy. I download delle miniature sono condivisi tra
la visualizzazione e la cache su disco, limitati a 4 MiB e quattro milioni di pixel
ciascuno, con al massimo tre tentativi automatici per gli errori temporanei. HTTP 429
rispetta `Retry-After` (secondi o data HTTP); gli errori di accesso rinnovano una volta
per commento la pagina originale dei commenti senza allegare i cookie dell'account
ai download delle immagini. Immagini rimosse, link invariati/scaduti, dati non
supportati e limiti esauriti mostrano un controllo per riprovare. Un elemento della
cache danneggiato può essere sostituito indipendentemente dalle altre immagini.
La cache su disco viene ridotta durante l'uso a 2.000 file e 128 MiB; le risposte
`no-store` vengono conservate solo per la visualizzazione corrente.

La diagnostica dell'intera applicazione è conservata in locale in
`$XDG_STATE_HOME/youtfast/logs` (di solito `~/.local/state/youtfast/logs` su Linux).
`youtfast.log` ruota in `youtfast.log.1`; ciascuno è limitato a 2 MiB. Questi file
coprono l'intera applicazione, comprese le categorie di errore del download delle
immagini e dei decoder. Non vengono caricati online. Su Unix la cartella è
accessibile solo al proprietario (0700) e i file di log sono 0600. Lo stesso
oscuramento per la privacy viene applicato prima di stderr e dei file, anche per la
diagnostica delle dipendenze. `--verbose` aggiunge le categorie della modalità di
animazione in cache/in streaming; non ci sono record di log per singolo fotogramma.
Il logging inizia quando questo binario gestisce la successiva istanza in
esecuzione; ricompilare non aggiorna un processo esistente. Per seguire il log
corrente su Linux (rotazione compresa):

```sh
tail -F "${XDG_STATE_HOME:-$HOME/.local/state}/youtfast/logs/youtfast.log"
```

Sulle piattaforme senza una cartella di stato XDG, i log usano la sottocartella
`logs` della cartella dei dati locali dell'applicazione della piattaforma per
YouTFast. Se l'apertura o la scrittura del file non riesce, la diagnostica continua
su stderr.

La riproduzione di GIF/WebP nei commenti usa un worker di decodifica e una texture
riutilizzata per ogni allegato visibile, con al massimo due allegati attivi. I
fotogrammi vengono ridotti per rientrare in 320×200 mantenendo le proporzioni. Le
animazioni brevi vengono messe in cache fino a circa 8 MiB; quelle più lunghe
proseguono in sequenza invece di conservare ogni fotogramma alle dimensioni
originali o di rifiutare l'intera clip oltre una soglia di numero di fotogrammi.
Le riserve controllate per fotogrammi, canvas e texture condividono un budget di
64 MiB; la gestione interna di decoder/librerie/driver e la normale cache delle
miniature sono memoria aggiuntiva del processo.
Un fotogramma che richiede più di 200 ms per la decodifica ferma l'ulteriore
decodifica automatica e mantiene un'anteprima; si tratta di un limite di lavoro
osservato, non dell'interruzione di un decoder nativo a metà fotogramma. Le
animazioni fuori dallo schermo vengono liberate, i controlli di pausa fermano le
richieste di fotogrammi e
i conteggi di ripetizione finiti della sorgente vengono rispettati. Gli errori di
decodifica dopo un fotogramma riuscito mantengono quel fotogramma come anteprima.
Blocchi esterni, contenuti rimossi o una sorgente inutilizzabile possono comunque
impedire un'anteprima o la riproduzione.

## Avvisi di nuovi caricamenti

Le notifiche di YouTube non hanno un'API, quindi YouTFast controlla invece le tue
iscrizioni a intervalli regolari (ogni 15 minuti per impostazione predefinita; 5, 15,
30 o 60 in Impostazioni). La campanella nella barra superiore apre un elenco
scorrevole senza lasciare la pagina corrente; lì puoi controllare di nuovo o
svuotarlo. Aprire la campanella segna gli avvisi correnti come visti, ma li mantiene
visibili durante quella visita.
I video trovati da un controllo mentre l'elenco è aperto vi compaiono e vengono
anch'essi segnati come visti.
La voce Notifiche della barra laterale apre comunque la pagina completa. I nuovi
caricamenti possono anche comparire come notifiche desktop. Sia i controlli sia le
notifiche desktop si possono disattivare in Impostazioni; il primo avvio fa solo
partire il conteggio, così una nuova installazione non ti sommerge di avvisi.

## Carattere dell'interfaccia

Impostazioni → Aspetto → Carattere dell'interfaccia. **Sistema** (il valore
predefinito) disegna YouTFast con il carattere dell'interfaccia del tuo desktop:
San Francisco su macOS, Segoe UI su Windows e, su Linux, il carattere che il tuo
desktop è impostato a usare (il carattere dell'interfaccia di GNOME, l'impostazione
di GTK o di KDE), altrimenti quello indicato da `system-ui` di fontconfig.
**Inter** è il carattere incluso in YouTFast, uguale su ogni computer. La modifica
si applica subito. Gli altri sistemi di scrittura (arabo, cinese, devanagari, ...)
vengono comunque disegnati con i caratteri installati sul computer.

Su Linux il menu elenca poi i caratteri installati sul computer (quelli che secondo
fontconfig possono disegnare testo in inglese): digita nel campo del menu per
restringere l'elenco e scegline uno per disegnare l'interfaccia con esso. Il testo
in grassetto e medio usa i pesi propri della famiglia, se li ha. Se un carattere
scelto viene poi rimosso, YouTFast disegna con Inter e indica quale carattere manca;
scegline un altro nello stesso menu. Windows e macOS offrono solo Sistema e Inter.

Se il carattere di sistema non può essere trovato o letto, YouTFast disegna con
Inter e il gruppo Aspetto lo segnala. **Segnala questo** apre il browser su una
nuova issue di GitHub con la versione di YouTFast, il sistema operativo, la sessione
desktop, il carattere che il desktop è impostato a usare e l'output di
`fc-match system-ui` già compilati; tu la leggi,
aggiungi il carattere usato dal tuo desktop e decidi se inviarla. YouTFast non
invia nulla da solo. Il tempo del player mantiene le cifre a larghezza fissa di
Inter con entrambe le scelte, così non tremola durante il conteggio.

## Temi

Impostazioni → Aspetto. **Automatico** segue in tempo reale il tema del desktop
Omarchy su Linux ed è scuro altrove. **Scuro** e **Chiaro** sono i colori propri di
YouTFast. Le otto tavolozze condivise (Catppuccin, Nord, Rosé Pine, Tokyo
Night e altre) vengono installate in `~/.config/youtfast/themes` al primo avvio
come semplici file JSON che puoi modificare, copiare o eliminare; aggiungi lì le tue
nello stesso formato (una `base` più colori con nome, vedi
[fastframe-theme](https://github.com/crmne/fastframe)). Una build dal sorgente non
attiva l'hook di Omarchy (`contrib/omarchy/youtfast-theme`). I pacchetti Linux
nativi lo includono per un'installazione facoltativa. Flatpak non attiva hook
dell'host né legge implicitamente i temi dell'host. Il tema nativo viene seguito
anche senza l'hook dove i file del tema sono accessibili.

## Informazioni e aggiornamenti

Impostazioni → Informazioni mostra la versione di YouTFast installata, la licenza e
il changelog incluso, e controlla in GitHub Releases la presenza di versioni stabili
più recenti all'avvio e ogni 12 ore mentre è aperto:

- **Notifica nuove versioni** (attivo per impostazione predefinita) invia una
  notifica desktop di sistema quando viene pubblicata una nuova release ed
  evidenzia Impostazioni nella barra laterale, con azioni rapide nel messaggio di
  notifica per aggiornare, aprire Informazioni o **Riavvia ora** quando è pronto.
- **Aggiornamenti automatici (minor e patch)** (disattivato per impostazione
  predefinita) scarica, verifica (SHA-256 rispetto al `checksums.txt` della release)
  e installa automaticamente in background gli aggiornamenti minor (`0.x.0`) e patch
  (`0.0.x`) per le installazioni scrivibili dall'utente (Linux nativo, Windows
  setup/portatile, macOS `.app`), poi mostra il changelog delle novità in
  Impostazioni → Informazioni.
- **Allineamento al metodo di installazione**: le installazioni `pacman` su Arch
  si aggiornano tramite `pkexec pacman -U` al clic (mai automaticamente in
  background), le installazioni Windows setup/portatili preparano e applicano
  l'aggiornamento in modo pulito al riavvio o alla chiusura per evitare DLL
  caricate incompatibili, e le installazioni Flatpak in sandbox inviano la
  notifica e offrono la copia del comando con un clic in Impostazioni →
  Informazioni.
- **Le versioni major (`x.0.0`)** non si installano mai automaticamente; richiedono
  un clic sul pulsante **Aggiorna** nell'app per confermare e installare la release.

## Aiuto

YouTube rifiuta i flussi di un video per qualche secondo dopo averli forniti
(fino al loro orario `available_at`). libmpv 0.42+ (con Lua) attende
quell'orario; con una libmpv precedente, YouTFast riprova ad aprire il flusso
rifiutato dopo 0, 1, 3 e 7 secondi, quindi un video può partire qualche secondo
più tardi che con la 0.42. Aggiornare libmpv alla 0.42+, oppure usare il
Flatpak, attende il flusso direttamente invece di riprovare.

Se la riproduzione mostra **"Impossibile riprodurre questo video. Il player non
ha identificato la causa. Prova a riaprire il video."**, il flusso era ancora
rifiutato dopo circa 11 secondi di tentativi, oppure qualcos'altro non ha
funzionato: segui i
[passaggi di risoluzione dei problemi dell'installazione](install.md#se-qualcosa-va-storto).

Leggi [CONTRIBUTING.md](../../../CONTRIBUTING.md) per le segnalazioni di bug e
[SECURITY.md](../../../SECURITY.md) per i problemi privati di account/sicurezza.
