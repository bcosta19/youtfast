# YouTFast-Benutzerhandbuch

[Zurück zur Übersicht](README.md) · [YouTFast installieren](install.md) · [Plattformdetails](../../distribution.md)

[English](../../user-guide.md) · [Português (Brasil)](../pt-BR/user-guide.md) · [Español](../es/user-guide.md) ·
[Français](../fr/user-guide.md) · **Deutsch** · [Italiano](../it/user-guide.md)

*Dies ist eine Übersetzung des [englischen Originals](../../user-guide.md). Bei Abweichungen gilt die englische Fassung.*

## Voraussetzungen

| Was | Wofür | Arch | Debian/Ubuntu |
|---|---|---|---|
| libmpv 0.42+ | Wiedergabe, beim Bauen gelinkt | `mpv>=0.42.0` | `libmpv-dev` (Version prüfen) |
| yt-dlp | Die Streams, die mpv abspielt, und erneute Wiedergabeversuche | `yt-dlp` | `yt-dlp` (aktuell halten) |
| Deno / passendes yt-dlp-EJS | Extraktions-Challenges von YouTube | `deno` und das EJS-Paket der Distribution | Siehe [yt-dlp-Einrichtung](https://github.com/yt-dlp/yt-dlp/wiki/EJS) |
| Rust 1.98 | Festgelegt in `rust-toolchain.toml` | `rustup` | `rustup` |
| python-secretstorage | Optional: Anmeldung über die Sitzung eines Chromium-basierten Browsers | `python-secretstorage` | `python3-secretstorage` |

mpv 0.42 oder neuer muss mit Lua gebaut sein (bei den üblichen Paketen ist das
der Fall): Es löst YouTube-Links über sein Skript `ytdl_hook` auf und wartet, bis
die Streams von yt-dlp verfügbar werden. Ältere Hooks können sie zu früh öffnen
und HTTP 403 erhalten. Halten Sie yt-dlp und seine JS-Laufzeitumgebung im `PATH`.
Prüfen Sie die Version des installierten **libmpv-Pakets**, nicht die Version
seiner Client-API oder die eines separat installierten mpv-Programms. Wenn Ihre
Distribution nur ein älteres libmpv bereitstellt, aktualisieren Sie es aus einer
vertrauenswürdigen Quelle oder verwenden Sie eine Flatpak-Version, die mpv 0.42+
enthält. Der Browser-Import benötigt außerdem `python3` mit dem Modul `yt_dlp`;
eine yt-dlp-Installation, die nur aus dem Programm besteht, liefert dieses
Python-Modul nicht mit.

## Ausführen

```sh
cargo run --locked --release
cargo run --locked --release -- "https://youtu.be/dQw4w9WgXcQ"   # open a link
cargo run --locked --release -- "rust egui"                      # or search
```

| Taste | Aktion |
|---|---|
| Leertaste, K | Wiedergabe oder Pause |
| ←, → | 5 s zurück oder vor |
| J, L | 10 s zurück oder vor |
| Strg+← / → (Option unter macOS) | Vorheriges / nächstes Kapitel |
| Pos1, Ende; 0–9 | Anfang / Ende; Position in Zehnerschritten in Prozent |
| ↑, ↓; M | Außerhalb des Players die Seite vertikal scrollen; im Player Lautstärke ±5 %; stummschalten (in Shorts wechseln ↑/↓ die Videos) |
| C; <, >; Komma, Punkt | Untertitel; Geschwindigkeit; vorheriges / nächstes Einzelbild bei Pause |
| Umschalt+N, Umschalt+P | Nächstes / vorheriges Element der Wiedergabequelle |
| I; T; Umschalt+A | Miniplayer; Kinomodus; Nur-Audio-Modus |
| F, Doppelklick | Vollbild |
| /; ? | Suche fokussieren; Hilfe zu Tastaturkürzeln öffnen |
| Alt+← / →; Maustaste zurück / vor | Zurück / vor navigieren (Alt-Tasten unter Linux/Windows) |
| Strg/Cmd+R | Aktuelle Seite neu laden |
| Esc | Menü/Hilfe/Countdown schließen, Vollbild verlassen oder zurückgehen |

Texteingaben und Menüs behalten ihre eigenen Tasten. Fokussierte Schaltflächen
belegen die Leertaste, und ein fokussierter Lautstärkeregler behält seine
Pfeiltasten. Außerhalb des Players scrollen ↑/↓ die aktive Seite vertikal, und
J/K verschieben die Auswahl (`g`/`G` springen zum Anfang oder Ende). Halten Sie
das Video gedrückt, um es vorübergehend mit 2× abzuspielen; beim Loslassen gilt
wieder die vorherige Geschwindigkeit. Das Player-Menü bietet außerdem
Wiederholen, Untertitelgröße/-hintergrund, das Kopieren eines Links mit
Zeitstempel und das Öffnen des Videos im Browser.

Die Suche nutzt direkt die InnerTube-Schnittstelle von YouTube, mit getrennten
Ergebnissen für Videos, Kanäle und Playlists. Videofilter umfassen Datum, Dauer,
Live-Status und Sortierung. Zurück/Vor stellt Seite, Suchbegriff, Filter,
Auswahl und Scrollposition wieder her. Karten haben Kanallinks, ein Menü und
Schnellaktionen für „Später ansehen“ und die Warteschlange. Die Warteschlange
lässt sich umsortieren oder kürzen; der Miniplayer ist ein kompaktes, rahmenloses
16:9-Videofeld in der unteren rechten Ecke (klicken, um zur Wiedergabeseite
zurückzukehren, oder mit der Maus darüberfahren für Wiedergabe/Pause und
Schließen). Die Wiedergabe einer Playlist geht über die zuerst geladene Seite
hinaus weiter. Der Geräteverlauf lässt sich filtern, und einzelne Einträge
können entfernt werden.

In einem breiten Fenster zeigt Einstellungen seine Gruppen in einem Verzeichnis
daneben an: Klicken Sie auf eine Gruppe, um zu ihr zu scrollen. Eine Zeile mit
einem Schalter kann an beliebiger Stelle angeklickt werden, nicht nur auf dem
Schalter.

Einstellungen → Seitenleiste legt fest, welche Schaltflächen der Seitenleiste
angezeigt werden und in welcher Reihenfolge (Schalter zum Ausblenden, Pfeile zum
Verschieben, Standard wiederherstellen zum Rückgängigmachen). Konto und
Einstellungen können nicht ausgeblendet werden. Videos, die ich mag, und
Playlists erscheinen weiterhin nur, wenn Sie angemeldet sind. Die Zifferntasten
1 bis 9 öffnen die Schaltflächen in der Reihenfolge, in der die Seitenleiste sie
zeigt.

Einstellungen → Wiedergabe → Miniplayer beim Scrollen hält ein Video im
Eck-Miniplayer sichtbar, wenn Sie daran vorbeiscrollen, um Kommentare zu lesen.
Standardmäßig ist es ausgeschaltet. Scrollen Sie zurück, erscheint wieder der
eingebettete Player; ein Klick auf den Miniplayer führt zum Video oben auf der
Seite zurück. Das Schließen des Miniplayers pausiert die Wiedergabe.

Einstellungen → Wiedergabe → Videopuffer wählt zwischen dem Standard-Demuxer-Cache
von mpv (**Groß — 150 MiB / 50 MiB**, die Voreinstellung, die für instabile
Verbindungen und sofortiges Spulen mehrere Minuten im Voraus puffert) und
**Klein — 50 MiB / 20 MiB** (um den RAM-Verbrauch bei etwa ~200 MB zu halten und
weniger Daten zu verbrauchen, wenn Videos früh übersprungen werden).
Der Nur-Audio-Modus (`Shift+A` oder die Kopfhörer-Schaltfläche in der
Player-Steuerung) streamt und dekodiert
nur die Tonspur (`ba/b` und `vid=no`) des aktuellen Videos und kehrt beim Öffnen
eines anderen Videos oder beim Neustart der App zum normalen Video zurück.

Die Wiedergabeseite zeigt außerdem die Beschreibung (Zeitstempel springen an die
Stelle), Kapitel und Kommentare, geladen über InnerTube. Kommentare und Antworten
zeigen auch Bild- sowie animierte GIF/WebP-Anhänge an. Anhänge nutzen den
begrenzten Vorschaubild-Cache, werden im Hintergrund dekodiert und nur animiert,
solange sie sichtbar sind.
Kanalseiten haben Tabs — Videos,
Shorts, Live, Playlists und Kanalinfo — sowie eine Suche in den eigenen Videos
des Kanals.

Kanalseiten enthalten ein Banner und eine Sortierung der Videos. Playlists zeigen
ihr Titelbild, ihre Metadaten und das gerade abgespielte Element. Shorts nutzen
eine Hochformat-Bühne mit Steuerung für Nächstes/Vorheriges und Navigation per
Mausrad. Die Oberfläche startet auf Englisch. Wählen Sie English, Português
(Brasil), Español, Français, Deutsch oder Italiano unter
Einstellungen → Darstellung → Sprache; die Änderung gilt sofort und bleibt
zwischen Starts erhalten, auch für Desktop-Benachrichtigungen. Ältere
Einstellungen fallen ebenfalls auf Englisch zurück. Videotitel, Kommentare und
Untertitel-Einstellungen behalten ihre eigenen Sprachen.

Die Wiedergabeseite kann ein durchsuchbares, synchronisiertes Transkript und
Vorschaubilder in der Zeitleiste anzeigen, wenn YouTube die nötigen öffentlichen
Metadaten liefert. Diese optionalen Anfragen starten bei Bedarf, laufen ohne
Konto und nutzen begrenzte Downloads. Fehlende Untertitel/Storyboards oder eine
Einschränkung auf YouTube-Seite können die Funktion unverfügbar machen; die
Wiedergabe läuft weiterhin über libmpv.

Für synthetische Vorschauen der Oberfläche bauen Sie mit `--features demo` und
führen aus:

```sh
cargo run --locked --features demo -- --demo --demo-view watch --demo-size 1280x800
cargo run --locked --features demo -- --demo --demo-view home --demo-light
cargo run --locked --features demo -- --demo --demo-view settings --demo-language pt-BR
cargo run --locked --features demo -- --demo --demo-view theater --demo-maximized
```

Der Demo-Modus öffnet ein isoliertes Fenster ohne Konten, Benutzerdateien,
Netzwerk, Wiedergabe oder Registrierung bei der Desktop-Mediensteuerung. Er zeigt
vorbereitete Zustände, nicht jede Interaktion mit dem Backend.
`--demo-capture target/example.png` nimmt dieses Fenster auf und schließt es.
`--demo-maximized` startet die Vorschau maximiert. In der Wiedergabeansicht
streckt `--demo-aspect 4:3` (auch `16:9` oder `21:9`) das synthetische Bild auf
ein anderes Format; `9:16` nutzt die Hochformat-Ersatzdarstellung. Die Ansichten
`watch`, `theater` und `fullscreen` zeigen diese Layouts. Die
Wiedergabe-Tastenbelegung folgt den
[von YouTube veröffentlichten Tastenkombinationen](https://support.google.com/youtube/answer/7631406),
mit den oben beschriebenen Anpassungen für Listen und Shorts.

Die Medientasten und -bedienfelder des Desktops (MPRIS, sodass auch `playerctl`
funktioniert) steuern die Wiedergabe, und ein zweiter Start übergibt einen Link
oder eine Suche an das bereits geöffnete Fenster, statt ein weiteres zu öffnen.

## Auf TV übertragen

Die TV-Auswahl unterstützt drei Wege. YouTube-TVs mit DIAL/Lounge nutzen native
Netzwerkfunktionen und benötigen kein Hilfsprogramm. Dazu gehören Modelle von
Samsung, LG, Roku, Fire TV und Android/Google TV, die YouTube über DIAL
bereitstellen; bietet ein TV sowohl DIAL als auch Google Cast, behält die Auswahl
beide Verbindungsoptionen. Lokale IP-Adressen unterscheiden TVs mit demselben
Namen. Jedes Gerät, dessen YouTube-App **Mit TV-Code verknüpfen** anzeigt (Apple
TV, Spielkonsolen, ein TV in einem anderen Netzwerk oder hinter einer
Gastnetz-Isolierung), kann stattdessen mit diesem Code verknüpft werden. Die
Unterstützung für Google Cast ist optional und nutzt
[`catt`](https://github.com/skorokithakis/catt), separat im `PATH` installiert.
Wenn Sie zum Beispiel bereits pipx verwenden, installieren Sie es mit
`pipx install catt`. Folgen Sie den Installationsanweisungen von catt für Ihr
Betriebssystem, prüfen Sie `catt --version` in der Umgebung, aus der YouTFast
gestartet wird, und starten Sie YouTFast neu, falls Sie den `PATH` geändert
haben. YouTFast installiert das Hilfsprogramm nie automatisch. Das Hilfsprogramm
und seine Python-Abhängigkeiten sind nicht in den Release-Archiven enthalten.
Flatpak kann kein beliebiges Hilfsprogramm des Host-Systems nutzen; diese
Integration ist für das Flatpak-Paket noch nicht freigegeben.

1. Schalten Sie den TV oder Chromecast ein und verbinden Sie ihn mit dem lokalen
   Netzwerk des Computers. Gastnetz-Isolierung und blockiertes Multicast können
   die Erkennung verhindern; die Suche wird über jede private
   Netzwerkschnittstelle gesendet, sodass VPNs und Container-Bridges das Netzwerk
   des TVs nicht verbergen. Unter Windows kann die Firewall die Antworten der TVs
   auf eine netzwerkweite Suche verwerfen: Erlauben Sie YouTFast, wenn Windows
   nachfragt, oder nutzen Sie die unten beschriebenen Optionen per IP oder
   TV-Code. Diese Version akzeptiert private/Link-Local-IP-Adressen. Google Cast
   nutzt Port 8009; YouTube DIAL nutzt den vom TV angekündigten Endpunkt.
2. Öffnen Sie ein YouTube-Video in YouTFast und klicken Sie in der
   Player-Steuerung neben **Nur Audio** auf **Auf TV übertragen**, oder nutzen Sie
   die Schaltfläche in der oberen Leiste.
   Die Erkennung läuft nur, wenn Sie die Auswahl zum ersten Mal öffnen oder
   **TVs suchen** drücken; es gibt keinen Netzwerk-Scan im Hintergrund.
   YouTube-TVs werden über SSDP und verifizierte DIAL-Endpunkte erkannt;
   AirPlay- und Google-Cast-Ankündigungen werden ebenfalls auf
   YouTube-DIAL-Unterstützung geprüft. Fehlt ein TV, geben Sie seine IP unter
   **YouTube-TV über IP suchen** ein und drücken Sie **TV suchen**. Die manuelle
   Suche fragt diese Adresse nach ihrem DIAL-Endpunkt und probiert dann die
   üblichen Pfade auf den Ports 8080, 8008 und 8060.
   Bei der Erkennung oder IP-Suche wird keine App gestartet.
   Um per Code zu verknüpfen, öffnen Sie YouTube auf dem TV, dann
   **Einstellungen → Mit TV-Code verknüpfen**, geben den 12-stelligen Code unter
   **TV mit Code verknüpfen** ein und drücken **TV verknüpfen**. Der Code wird
   einmal verwendet und nicht gespeichert; der verknüpfte TV wird über die Server
   von YouTube erreicht und bleibt in der Liste, bis YouTFast beendet wird.
3. Wählen Sie das Gerät. YouTFast sendet den kanonischen Videolink und die lokale
   Position, abgerundet auf ganze Sekunden. Live-Videos fordern den aktuellen
   Live-Stand an. Der lokale Player spielt weiter, bis der TV das angeforderte
   Video als laufend meldet, und wartet dabei bis zu einer Minute auf Werbung und
   die Kontoauswahl des TVs. Bei DIAL tritt die Wahl des TVs dessen YouTube-App
   bei, wenn sie bereits geöffnet ist, oder öffnet sie, und baut eine
   Lounge-Verbindung im Arbeitsspeicher auf; eine untätige Verbindung wird
   automatisch erneuert. Fordert der TV Sie auf, sich anzumelden, ein Konto zu
   wählen oder als Gast zuzusehen, treffen Sie die Wahl am TV; YouTFast wickelt
   diese Anmeldung nie ab.
   Eine verspätete Bestätigung pausiert nie ein anderes lokales
   Video, auch nicht eine neu geöffnete Wiedergabe desselben Videos.
4. Nutzen Sie die ausdrücklichen TV-Steuerelemente des Bedienfelds: Wiedergabe,
   Pause, zehn Sekunden zurück/vor, Lautstärke um zehn Prozentpunkte
   leiser/lauter und **Auf TV stoppen**. Befehle werden einzeln gesendet; das
   Bedienfeld synchronisiert den TV-Status nicht fortlaufend.
5. Das Schließen des Bedienfelds blendet es nur aus. Öffnen Sie es über die obere
   Leiste erneut, um dasselbe Ziel zu steuern. **Trennen; TV weiterspielen
   lassen** oder das Beenden von YouTFast lässt die Wiedergabe auf dem TV laufen
   und verwirft die Steuersitzung im Arbeitsspeicher. **Auf TV stoppen** bittet
   einen Cast-Empfänger, anzuhalten, oder schließt die YouTube-App per DIAL; ein
   TV, der das Schließen nicht erlaubt, oder einer, der per Code verknüpft ist,
   stoppt das Video. Keine der beiden Aktionen
   setzt den lokalen Player automatisch fort.

Das Bedienfeld ist eine separate Fernbedienung: Die Steuerelemente des lokalen
Players, die Tastaturkürzel und die Desktop-Medientasten steuern weiterhin den
Computer. Der Punkt an der Cast-Schaltfläche bedeutet, dass ein Ziel gemerkt ist,
nicht, dass die Wiedergabe noch läuft. Eine erfolgreiche erste Bestätigung ist
eine Momentaufnahme; Änderungen am TV werden nicht abgefragt. Eine
Zeitüberschreitung oder ein Fehler kann auftreten, nachdem der TV bereits
gestartet hat: Prüfen Sie den TV, bevor Sie erneut senden. Das Bedienfeld behält
ein unsicheres Ziel, sodass Sie es trotzdem stoppen oder trennen können.

Google-Cast-Ziele sind Chromecast, Google TV Streamer
sowie kompatible Google-TV-, Android-TV-, LG-, Samsung- und andere TVs mit
aktiviertem Cast. Die Unterstützung hängt vom genauen Modell und der Firmware ab,
nicht von der Marke. Bekannte Google-Lautsprecher (Home, Nest Mini, Nest Audio,
Chromecast Audio) werden aus der Liste ausgeschlossen; andere reine
Audio-Cast-Geräte können trotzdem erscheinen und sind keine Videoziele. Ein TV,
der seine Lautstärke nicht meldet (zum Beispiel mit Ton über einen
HDMI-ARC-Receiver), regelt die Lautstärke über seine eigene Fernbedienung.
DIAL-Ziele benötigen eine funktionierende YouTube-App, die eine
Bildschirmkennung bereitstellt, und Zugang zu YouTube Lounge. AirPlay ist nur ein
Hinweis bei der Erkennung; Medien werden nicht über AirPlay, Miracast oder DLNA
gesendet. Die Freigabe für natives Windows/macOS und Flatpak steht noch aus. Die
Lounge-API ist undokumentiert und kann sich unabhängig von YouTFast ändern; ihre
Protokollreferenzen sind die Projekte ytcast und casttube.

Die YouTube-App und das Konto des TVs übernehmen die Wiedergabe, einschließlich
Werbung und Zugriffsbeschränkungen. YouTFast überträgt weder seine Kontositzung
noch Browser-Cookies oder die lokale Mediathek. Wenn
**Sponsoren überspringen (SponsorBlock)** aktiviert ist, prüft YouTFast außerdem
die TV-Position des gesendeten Videos und sendet Sprungbefehle aus der Ferne über
DIAL/Lounge oder Google Cast (`catt`). Es nutzt dieselben automatischen
Kategorien wie die lokale Wiedergabe: Sponsoren, Eigenwerbung und
Interaktionserinnerungen. Lassen Sie YouTFast laufen und verbunden; Trennen oder
das Deaktivieren von SponsorBlock beendet weitere automatische Sprünge.
Das gilt für das ausdrücklich gesendete Video, nicht für nachfolgende Videos, die
die automatische Wiedergabe des TVs auswählt. Livestreams, pausierte Wiedergabe
und fehlende Positionsdaten werden nicht übersprungen. Die Prüfungen laufen in
der Nähe eines Segments etwa alle zwei Sekunden und bis zu zehn Sekunden
auseinander, wenn das nächste weit entfernt ist. Die Positionsabfrage am TV und
der Start des Hilfsprogramms können einen Sprung verzögern; die letzten zwei
Sekunden eines Segments bleiben unangetastet. Werbeereignisse von Lounge
unterdrücken Sponsor-Sprünge; `catt` liefert kein zuverlässiges separates
Werbekennzeichen, daher verlässt sich Cast auf den Wiedergabestatus und die
Sprungfähigkeit des Empfängers. YouTube-Werbung wird dadurch nicht blockiert.
Die Freigabe mit physischen TVs/Chromecasts und auf nativen Plattformen steht
noch aus.

Lokale Untertitel-Einstellungen, lokale automatische Wiedergabe/Warteschlange und
die Regeln der Verlaufssynchronisierung des Kontos werden nicht an den TV
übertragen;
YouTube auf dem TV kann einen eigenen Verlauf aufzeichnen. Erkennungsergebnisse
und der ausgewählte TV werden nur im Arbeitsspeicher gehalten. Rohe Ausgaben und
Fehler des Hilfsprogramms werden nie angezeigt oder protokolliert, auch nicht bei
ausführlicher Protokollierung.

## Ihre Mediathek, mit oder ohne Konto

**Startseite** zeigt das Neueste aus Ihren Abos — den Feed des Kontos oder die
RSS-Kanäle des Geräts —, nach Tagen gruppiert und seitenweise: Scrollen Sie ans
Ende, und es kommt mehr. Shorts liegen in einer Leiste über der Liste: Die des
Kontos werden von YouTube markiert, die der Gerätekanäle stammen aus deren
Shorts-Tabs.

**Ohne Konto** speichert YouTFast Abos, Wiedergabeverlauf (mit der Stelle, an der
jedes Video verlassen wurde, um es fortzusetzen) und „Später ansehen“ auf diesem
Computer, in `~/.local/share/youtfast/library.json`, und seine Einstellungen
(Videoqualität, Lautstärke) in `~/.config/youtfast/settings.json`. Der Feed der
Startseite stammt aus dem RSS jedes abonnierten Kanals. Eine bestehende
Mediathek übernehmen Sie aus [Google Takeout](https://takeout.google.com): Wählen
Sie YouTube und YouTube Music, mit JSON als Format für den Verlauf, und
importieren Sie `subscriptions.csv` und `watch-history.json`. Nichts davon
berührt ein Google-Konto. Importe akzeptieren Dateien bis 64 MiB und 20.000
eindeutige Abos und behalten die 1.000 neuesten eindeutigen Verlaufseinträge.
Einlesen und Zusammenführen laufen im Hintergrund. Ändert sich die Mediathek
während des Zusammenführens, wiederholen Sie den Import; diese neueren Änderungen
werden nie durch einen älteren Import-Stand ersetzt.

**History sync** (Verlaufssynchronisierung) unter Einstellungen → Verlauf gilt, solange Sie
angemeldet sind:

- **Separate** (Getrennt; die Voreinstellung) hält die Listen Konto und Dieses Gerät
  getrennt.
- **Merged in YouTFast** (in YouTFast zusammengeführt) öffnet den Verlauf auf **Alle** und kombiniert
  beide Listen ohne doppelte Videos. **Konto** und **Dieses Gerät** bleiben als
  Filter verfügbar. Die Suche filtert die kombinierte Liste; lokale
  Fortsetzungspositionen und Gesehen-Markierungen haben Vorrang. Nichts wird an
  YouTube gesendet.
- **Merged and sent to YouTube** (zusammengeführt und an YouTube gesendet) meldet außerdem Videos, deren
  Wiedergabe hier tatsächlich beginnt, nach zwei Sekunden aktiver Wiedergabe an
  das Konto, das beim Start der Wiedergabe aktiv ist. Schnell übersprungene
  Shorts werden nicht gemeldet. Dadurch werden Wiedergabeberichte aus Ihrem
  Konto an Google gesendet; Google kann dies als automatisierten Zugriff
  einstufen. Fehlgeschlagene Meldungen unterbrechen die Wiedergabe nicht. Die
  Kontoliste wird aktualisiert, wenn der Verlauf nach einer erfolgreichen Meldung
  das nächste Mal geöffnet wird.

Die zusammengeführte Liste behält die Tagesüberschriften von YouTube. Da der
Kontoverlauf Tagesbezeichnungen statt genauer Wiedergabezeiten hat, ist die
Reihenfolge zwischen den beiden Quellen eine Annäherung; unbekannte
Tagesbezeichnungen behalten die Reihenfolge des Kontos. Die Schaltfläche zum
Entfernen ist nur für Videos verfügbar, die auf diesem Gerät gespeichert sind,
und löscht nur diesen lokalen Eintrag. Der Kontoverlauf bleibt für das Löschen
schreibgeschützt. Die Einstellung hat keine Wirkung, solange Sie abgemeldet sind.

Eine unlesbare Mediathek- oder Einstellungsdatei wird gesichert, bevor
Standardwerte gespeichert werden können. Ein Lese- oder Sicherungsfehler stoppt
den Start, statt das Original zu überschreiben. Speichervorgänge beim Beenden
warten auf laufende Speichervorgänge und verhindern, dass ältere, in der
Warteschlange stehende Stände den endgültigen Zustand ersetzen.

**Mit Ihren YouTube-Konten** wird jedes Konto benannt und unter einem eigenen
Eintrag im Schlüsselbund gespeichert: Fügen Sie mehrere hinzu, wechseln Sie auf
der Kontoseite oder mit `a`, und richten Sie eine Browser-Anmeldung auf ein
Profil aus (`Profile 1`, `Default`, …). Feed, Abos, Verlauf, Später ansehen,
Videos, die ich mag, und Playlists stammen dann aus dem Konto. YouTube bietet
keine Anmeldung für Apps wie diese, daher nutzt YouTFast die Sitzung, die ein
Browser bereits hat:

- **Die Anmeldung Ihres Browsers**: YouTFast liest die YouTube-Cookies des
  Browsers über yt-dlp und liest sie erneut, sobald sie ablaufen.
- **Eine Datei cookies.txt**: aus einem privaten Fenster exportiert, wodurch die
  eigene Sitzung des Browsers außen vor bleibt.

Die Sitzung liegt nur im Schlüsselbund des Systems (Secret Service, Keychain,
Credential Manager), nie in einer Datei oder im Protokoll. **Wird ein Konto auf
diese Weise genutzt, kann Google dies als automatisierten Zugriff einstufen**:
Das Konto könnte zur Verifizierung aufgefordert, abgemeldet oder gesperrt werden.
Ein Zweitkonto ist die sichere Wahl. Abonnieren und „Später ansehen“ auf dem
Gerät wirken lokal. Das Bewerten eines Videos oder das Bearbeiten einer
Konto-Playlist über die Wiedergabeseite ändert das ausgewählte Konto, mit
Rücknahme, falls YouTube es ablehnt. Die Verlaufssynchronisierung kann zusätzlich
gestartete Wiedergaben an dieses Konto melden, wenn Sie ausdrücklich
**Merged and sent to YouTube** wählen; die Voreinstellung sendet
keine Wiedergabeberichte.

Kontowechsel leeren private Listen und Caches sofort und schließen geöffnete
Menüs. Ausstehende Schreibvorgänge werden vor dem Wechsel in ihrer
ursprünglichen Sitzung abgeschlossen, und verspätete Antworten können die
Oberfläche des neuen Kontos nicht aktualisieren. Ein Ablauf der Sitzung oder ein
Anmeldefehler verwirft außerdem private Antworten aus demselben ausstehenden
Stapel. Kein Client-Cache wird auf den Datenträger geschrieben; nur der
Schlüsselbund-Eintrag des ausgewählten Kontos kann seine Sitzung wiederherstellen.

Protokolle nutzen feste Diagnosekategorien statt Kontonamen, Video-IDs,
Browserprofilen, URLs oder externer Fehlertexte. Diese Schwärzung gilt auch für
Abhängigkeiten und für `--verbose` / `RUST_LOG`; ausführlichere Protokollierung
hebt den Datenschutz nicht auf. Downloads von Vorschaubildern werden zwischen
Anzeige und Datenträger-Cache geteilt, sind auf je 4 MiB und vier Millionen Pixel
begrenzt und haben höchstens drei automatische Versuche bei vorübergehenden
Fehlern. HTTP 429 berücksichtigt `Retry-After` (Sekunden oder HTTP-Datum);
Zugriffsfehler erneuern die ursprüngliche Kommentarseite einmal pro Kommentar,
ohne Konto-Cookies an Bild-Downloads anzuhängen. Entfernte Bilder,
unveränderte/abgelaufene Links, nicht unterstützte Daten und ausgeschöpfte
Grenzen zeigen ein Steuerelement zum erneuten Versuchen. Ein beschädigter
Cache-Eintrag kann unabhängig von anderen Bildern ersetzt werden.
Der Datenträger-Cache wird während der Nutzung auf 2.000 Dateien und 128 MiB
gekürzt; `no-store`-Antworten werden nur für die aktuelle Anzeige behalten.

Anwendungsweite Diagnosedaten werden lokal unter
`$XDG_STATE_HOME/youtfast/logs` gespeichert (unter Linux normalerweise
`~/.local/state/youtfast/logs`). `youtfast.log` rotiert nach `youtfast.log.1`;
jede Datei ist auf 2 MiB begrenzt. Diese Dateien decken die gesamte Anwendung ab,
einschließlich der Fehlerkategorien für Bild-Downloads und Decoder. Sie werden
nicht hochgeladen. Unter Unix ist das Verzeichnis nur für den Eigentümer
zugänglich (0700), und Protokolldateien haben 0600. Derselbe
Datenschutz-Schwärzer läuft vor stderr und den Dateien, auch für Diagnosedaten
von Abhängigkeiten. `--verbose` ergänzt Kategorien für den gecachten/streamenden
Animationsmodus; es gibt keine Protokolleinträge pro Einzelbild. Die
Protokollierung beginnt, sobald dieses Programm das nächste Mal eine laufende
Instanz stellt; ein Neubau aktualisiert keinen bestehenden Prozess. So verfolgen
Sie unter Linux das aktuelle Protokoll (einschließlich Rotation):

```sh
tail -F "${XDG_STATE_HOME:-$HOME/.local/state}/youtfast/logs/youtfast.log"
```

Auf Plattformen ohne XDG-Statusverzeichnis nutzen Protokolle das Unterverzeichnis
`logs` im lokalen Anwendungsdatenverzeichnis der Plattform für YouTFast. Schlägt
das Öffnen oder Schreiben der Datei fehl, laufen die Diagnosedaten weiter über
stderr.

Die Wiedergabe von GIF/WebP in Kommentaren nutzt einen Decoder-Worker und eine
wiederverwendete Textur pro sichtbarem Anhang, mit höchstens zwei aktiven
Anhängen. Einzelbilder werden unter Beibehaltung des Seitenverhältnisses auf
320×200 verkleinert. Kurze Animationen werden bis etwa 8 MiB gecacht; längere
laufen sequenziell weiter, statt jedes Einzelbild in Originalgröße zu behalten
oder den ganzen Clip ab einer Einzelbild-Schwelle abzulehnen. Kontrollierte
Reservierungen für Einzelbilder, Zeichenflächen und Texturen teilen sich ein
Budget von 64 MiB; die Verwaltung durch Decoder/Bibliothek/Treiber und der
normale Vorschaubild-Cache sind zusätzlicher Prozessspeicher.
Ein Einzelbild, dessen Dekodierung länger als 200 ms dauert, beendet die weitere
automatische Dekodierung und behält eine Vorschau; dies ist eine beobachtete
Arbeitsgrenze, kein Unterbrechen eines nativen Decoders mitten in einem
Einzelbild. Animationen außerhalb des sichtbaren Bereichs werden freigegeben,
Pause-Steuerelemente stoppen Einzelbild-Anfragen, und
endliche Wiederholungszahlen der Quelle werden eingehalten. Dekodierfehler nach
einem erfolgreichen Einzelbild behalten dieses Bild als Vorschau. Externe
Sperren, entfernte Medien oder eine unbrauchbare Quelle können eine Vorschau
oder Wiedergabe dennoch verhindern.

## Benachrichtigungen über neue Uploads

Die eigenen Benachrichtigungen von YouTube haben keine API, daher prüft YouTFast
Ihre Abos stattdessen per Zeitgeber (standardmäßig alle 15 Minuten; 5, 15, 30
oder 60 in den Einstellungen). Die Glocke in der oberen Leiste öffnet eine
scrollbare Liste, ohne die aktuelle Seite zu verlassen; dort können Sie erneut
prüfen oder sie leeren. Das Öffnen der Glocke markiert die aktuellen
Benachrichtigungen als gesehen, lässt sie während dieses Besuchs aber sichtbar.
Videos, die eine Prüfung findet, während die Liste geöffnet ist, erscheinen dort
und werden ebenfalls als gesehen markiert.
Der Eintrag Benachrichtigungen in der Seitenleiste öffnet weiterhin die
vollständige Seite. Neue Uploads können auch als Desktop-Benachrichtigungen
erscheinen. Sowohl die Prüfungen als auch die Desktop-Benachrichtigungen lassen
sich in den Einstellungen abschalten; der erste Start setzt nur die Uhr in Gang,
damit eine frische Installation Sie nicht überflutet.

## Schriftart der Oberfläche

Einstellungen → Darstellung → Schriftart der Oberfläche. **System** (die
Voreinstellung) zeichnet YouTFast mit der eigenen Oberflächenschrift Ihres
Desktops: San Francisco unter macOS, Segoe UI unter Windows und unter Linux die
Schrift, die Ihr Desktop verwendet (die Oberflächenschrift von GNOME, die
Einstellung von GTK oder KDE), andernfalls das, was `system-ui` von fontconfig
benennt.
**Inter** ist die mit YouTFast ausgelieferte Schrift, auf jedem Computer gleich.
Die Änderung gilt sofort. Andere Schriftsysteme (Arabisch, Chinesisch,
Devanagari, ...) werden in beiden Fällen mit den auf dem Computer installierten
Schriften gezeichnet.

Unter Linux listet das Menü dann die auf dem Computer installierten Schriften auf
(diejenigen, die laut fontconfig englischen Text darstellen können): Tippen Sie
in das Feld des Menüs, um die Liste einzugrenzen, und wählen Sie eine, um die
Oberfläche damit zu zeichnen. Fetter und mittelstarker Text nutzt die eigenen
Schnitte der Schriftfamilie, sofern vorhanden. Wird eine gewählte Schrift später
entfernt, zeichnet YouTFast mit Inter und nennt die fehlende Schrift; wählen Sie
im selben Menü eine andere. Windows und macOS bieten nur System und Inter.

Kann die Systemschrift nicht gefunden oder gelesen werden, zeichnet YouTFast mit
Inter, und die Gruppe Darstellung weist darauf hin. **Melden** öffnet Ihren
Browser mit einem neuen GitHub-Issue, in dem die YouTFast-Version, Ihr
Betriebssystem, die Desktop-Sitzung, die von Ihrem Desktop verwendete Schrift und
die Ausgabe von `fc-match system-ui` bereits eingetragen sind; Sie lesen es,
ergänzen die Schrift, die Ihr Desktop verwendet, und entscheiden, ob Sie es
absenden. YouTFast sendet nichts von selbst. Die Zeitanzeige des Players behält
bei beiden Optionen die Ziffern fester Breite von Inter, damit sie beim Zählen
nicht wackelt.

## Designs

Einstellungen → Darstellung. **Automatisch** folgt unter Linux live dem Design des
Omarchy-Desktops und ist anderswo dunkel. **Dunkel** und **Hell** sind die
eigenen Farben von YouTFast. Die acht gemeinsamen Farbpaletten (Catppuccin, Nord,
Rosé Pine, Tokyo Night und weitere) werden beim ersten Start als einfache
JSON-Dateien in `~/.config/youtfast/themes` installiert, die Sie bearbeiten,
kopieren oder löschen können; fügen Sie dort eigene im selben Format hinzu (eine
`base` plus benannte Farben, siehe
[fastframe-theme](https://github.com/crmne/fastframe)). Ein Build aus dem
Quellcode aktiviert den Omarchy-Hook (`contrib/omarchy/youtfast-theme`) nicht.
Native Linux-Pakete enthalten ihn zur optionalen Installation. Flatpak aktiviert
keine Hooks des Host-Systems und liest dessen Designs nicht stillschweigend. Das
native Folgen des Designs funktioniert auch ohne den Hook, wo die Design-Dateien
zugänglich sind.

## Info und Updates

Einstellungen → Kanalinfo zeigt die installierte YouTFast-Version, die Lizenz und
das mitgelieferte Änderungsprotokoll und prüft GitHub Releases beim Start sowie
alle 12 Stunden im laufenden Betrieb auf neuere stabile Versionen:

- **Über neue Versionen benachrichtigen** (standardmäßig an) sendet eine
  Desktop-Benachrichtigung des Systems, wenn eine neue Version veröffentlicht
  wird, und hebt Einstellungen in der Seitenleiste hervor, mit Schnellaktionen in
  der Benachrichtigung zum Aktualisieren, zum Öffnen von Kanalinfo oder zum
  **Jetzt neu starten**, sobald das Update bereit ist.
- **Automatische Updates (Minor und Patch)** (standardmäßig aus) lädt Minor-
  (`0.x.0`) und Patch-Updates (`0.0.x`) für vom Benutzer beschreibbare
  Installationen (natives Linux, Windows-Setup/-Portable, macOS `.app`)
  automatisch herunter, prüft sie (SHA-256 gegen die `checksums.txt` der Version)
  und installiert sie im Hintergrund; anschließend zeigt es unter
  Einstellungen → Kanalinfo das Änderungsprotokoll mit den Neuerungen an.
- **Abstimmung auf die Installationsmethode**: Arch-`pacman`-Installationen
  aktualisieren sich per Klick über `pkexec pacman -U` (nie unbeaufsichtigt im
  Hintergrund), Windows-Setup-/-Portable-Installationen bereiten das Update vor
  und wenden es sauber beim Neustart oder Beenden an, damit geladene DLLs nie
  inkonsistent bleiben, und Flatpak-Installationen in der Sandbox benachrichtigen
  und bieten unter Einstellungen → Kanalinfo das Kopieren des Befehls per Klick.
- **Hauptversionen (`x.0.0`)** werden nie automatisch installiert; sie erfordern
  einen Klick auf die Schaltfläche **Aktualisieren** in der App, um die
  Version zu bestätigen und zu installieren.

## Hilfe

YouTube verweigert die Streams eines Videos einige Sekunden lang, nachdem es sie
ausgegeben hat (bis zu ihrem Zeitpunkt `available_at`). libmpv 0.42+ (mit Lua)
wartet diesen Zeitpunkt ab; mit älterem libmpv versucht YouTFast den
abgewiesenen Stream stattdessen nach 0, 1, 3 und 7 Sekunden erneut zu öffnen,
sodass ein Video einige Sekunden später als mit 0.42 starten kann. Mit libmpv
0.42+ oder dem Flatpak wird direkt auf den Stream gewartet statt wiederholt.

Zeigt die Wiedergabe **„Dieses Video konnte nicht abgespielt werden. Die
Ursache ist unbekannt. Versuchen Sie, das Video erneut zu öffnen.“**, wurde der
Stream auch nach etwa 11 Sekunden Wiederholungen abgewiesen, oder etwas anderes
ist fehlgeschlagen: Folgen Sie den
[Schritten zur Fehlerbehebung bei der Installation](install.md#wenn-etwas-schiefgeht).

Lesen Sie [CONTRIBUTING.md](../../../CONTRIBUTING.md) für Fehlerberichte und
[SECURITY.md](../../../SECURITY.md) für vertrauliche Konto-/Sicherheitsprobleme.
