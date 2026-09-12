# Änderungen

[English](CHANGELOG.md)

Jede Veröffentlichung bis 1.0 ist eine Vorabversion, und das ist eine Aussage
über den Zustand des Programms, keine Formalie: Es tut, was es sagt, es ist
geprüft, und es ist nicht fertig. Die Update-Benachrichtigung in AmberBeam
liest Vorabversionen genau deshalb.

## 0.1.5

AmberBeam ganz ohne Fenster, und Dateien dort bearbeitet, wo sie liegen.

### Hinzugefügt

- **Ein Container.** Dasselbe Programm ohne Fenster: Es läuft auf einer
  Maschine, und du erreichst es im Browser. Derselbe Kern, dieselben Befehle,
  dieselbe Oberfläche — nur der Weg vom Fenster zum Kern ändert sich, und die
  Oberfläche merkt davon nichts. Nützlich, wenn Übertragungen weiterlaufen
  sollen, nachdem der Laptop zugeklappt ist.

  Ein Passwort statt Benutzerkonten, und kein eigenes TLS: Ein Reverse Proxy
  davor erneuert Zertifikate, was ein Container nicht kann, und läuft auf
  dieser Maschine ohnehin schon. Gespeicherte Passwörter kommen in eine
  versiegelte Datei unter einer Passphrase, weil es dort draußen keinen
  Schlüsselbund gibt — und eine falsche Passphrase hält das Programm an,
  statt leer zu starten und zu überschreiben, was es nicht lesen konnte. Siehe
  [Als Container betreiben](docs/container.de.md).
- **Das Desktop-Programm kann einen Dienst bedienen.** Dasselbe Fenster,
  dieselben Tasten, aber die Übertragungen passieren drüben und laufen ohne es
  weiter. Unter **Server**.
- **Dateien dort bearbeiten, wo sie liegen.** Rechtsklick auf eine Datei auf
  dem Server, **Remote bearbeiten** — eine Kopie kommt herunter, geht auf, und
  jedes Speichern geht wieder hinauf.

  Was dabei stimmen muss, ist nicht das Bearbeiten. Eine Datei, die kein Text
  ist, wird an ihren Bytes erkannt und abgelehnt, wie sie auch heißt. Eine
  Datei, die kein UTF-8 ist, wird als Latin-1 gelesen und als Latin-1
  zurückgeschrieben, denn sie als UTF-8 zu lesen und zu speichern schreibt
  jeden Umlaut darin neu, und es fällt erst viel später auf — dasselbe gilt für
  ein Byte Order Mark und für CR-LF-Zeilenenden. Wie die Datei aussah, als die
  Kopie genommen wurde, wird gemerkt: Über den Nachmittag eines anderen zu
  schreiben wird gefragt, nicht getan.

  Eine Tabelle unter **Einstellungen → Bearbeiten** sagt, welche Dateiarten
  bearbeitet werden dürfen und was jede öffnet: AmberBeams eigener Editor, was
  das System nimmt, oder ein Programm deiner Wahl. Eine Datei in einem fremden
  Programm wird beobachtet, und was es speichert, geht von selbst hinauf. Siehe
  [Eine Datei dort bearbeiten, wo sie liegt](docs/editing.de.md).
- **Ein Papierkorb auf dem Server**, pro gespeichertem Server. Gelöschte
  Dateien wandern in ein Verzeichnis deiner Wahl, benannt nach dem Moment des
  Wegwerfens — nach Namen sortiert ist der Papierkorb damit nach Zeit sortiert.
  Löschen darin ist endgültig. Scheitert das Verschieben, wird nichts gelöscht
  und der Papierkorb für diesen Server abgeschaltet: Einer, der nichts annehmen
  kann, ist schlimmer als keiner.
- **Dort weitermachen, wo ein Server verlassen wurde**, ein Haken pro Server.
  Die Serverseite öffnet in dem Verzeichnis, das zuletzt zu sehen war, statt im
  eingetragenen. Nicht mehr da? Dann das eingetragene, kommentarlos: Ein Fehler
  über eine Bequemlichkeit, um die niemand gebeten hat, ist keiner, den man
  sehen will.
- **Beide Seiten können sich gemeinsam bewegen.** Ein Verzeichniswechsel auf
  einer Seite nimmt die andere mit, entlang des Pfads relativ zum Start der
  Kopplung.
- **Die Oberfläche lässt sich vergrößern**, in vier Stufen, unter Darstellung.
- **Zur Warteschlange hinzufügen, ohne zu starten** — zum Sammeln.
- **Die Überschreiben-Frage einmal beantworten.** Die Antwort gilt jetzt für
  diese Datei, den Rest dieses Laufs oder jeden Lauf ab jetzt — einschließlich
  der Dateien, an die noch niemand gedacht hat, was sie bedeuten muss, solange
  ein Ordner noch durchlaufen wird. Vierzigmal dieselbe Frage zu beantworten
  ist keine Zustimmung.
- **Fertige Übertragungen können sich selbst aufräumen**, nach einem Moment,
  neben dem Pause-Knopf. Aus als Voreinstellung: Eine Warteschlange, die sich
  selbst leert, ist genau so lange ordentlich, bis jemand wissen will, ob das
  Gestartete tatsächlich passiert ist.
- **Ordner und Dateien haben eigene Symbole**, in der Liste und im Baum. Das
  Bild im README zeigt sie seit der ersten Fassung, und das Programm zeichnete
  ein Dreieck und einen Punkt.
- **Ein langsamer zweiter Klick benennt um**, wie überall sonst auch.
- **Der Weg nach oben steht wieder in der Liste** — die `..`-Zeile — und eine
  Reihe von Zeilen lässt sich mit Shift markieren, per Klick oder mit den
  Pfeiltasten.

### Behoben

- **Über IPv6 war per FTP keine Übertragung möglich.** `PASV` kann nur eine
  IPv4-Adresse nennen, also fragte jede Datenverbindung über IPv6 nach etwas,
  das das Protokoll nicht ausdrücken kann. Wo der Server es anbietet, wird
  `EPSV` benutzt. Eine Verbindung, die nach der Anmeldung stehen blieb, gibt
  jetzt außerdem auf und sagt es, statt ewig zu warten.
- **Ein Server, der TLS verlangt, wurde als falsches Passwort gemeldet** — und
  schickte Leute los, ein Passwort zu prüfen, das nie angesehen wurde.
- **Der Schlüsselbund fragte einmal pro Server**, jedes Mal beim Öffnen der
  Liste, weil „gibt es ein Passwort" hieß, es zu holen. Das beantwortet jetzt
  der Eintrag selbst, und geholt wird erst, wenn sich etwas verbindet.
- **Der Container konnte weder verbinden noch etwas einreihen.** Beide Brücken
  fragten dieselben Befehle unter denselben Namen, und eine schickte die
  Argumente in einer Form, die der Kern nicht liest. Alle Prüfungen grün,
  gescheitert erst beim Klick auf Verbinden. Der Wächter vergleicht jetzt, was
  jede Brücke schickt, nicht nur, wie sie es nennt.
- **Ein vom Server abgelehntes Schreiben meldete, die Datei ließe sich nicht
  lesen** — falsch in der Richtung und in der Ursache, und es schickt jemanden
  auf die Suche nach einer Datei, die genau dort liegt.
- **Eine Seite verlor beim zweiten Verbindungsversuch ihren Server** — nach dem
  Annehmen eines Host-Keys oder Zertifikats oder dem Nachtippen eines nicht
  gespeicherten Passworts. Mit ihm ging die Regel darüber, was Löschen heißt.
- **Das ganze Fenster wurde bernsteinfarben hinter einem Kontextmenü.** Was den
  Klick außerhalb eines Menüs auffängt, ist ein Knopf, und eine Regel für die
  Knöpfe darin erwischte auch ihn.
- **Die Knöpfe, die im Serverformular die Arbeit abschließen, bleiben sichtbar**
  statt irgendwo unterhalb des Fensterrands zu liegen.

## 0.1.4

Updates aus dem Programm heraus, und ein Fenster, das sich wieder herrichten
lässt.

### Neu

- **Updates installieren sich selbst.** Der Knopf in der oberen Leiste zeigt,
  was die neue Fassung über sich sagt, und von dort wird sie geholt, geprüft
  und installiert — ohne Browser, ohne Datei im Download-Ordner, ohne
  Installationsprogramm, das man suchen muss.

  Das Prüfen ist der Punkt, nicht die Bequemlichkeit. Ein Programm, das sich
  selbst ersetzt, muss „woher kommt das" beantworten können, ohne dem Netz zu
  vertrauen. Jedes Update ist mit einem Schlüssel signiert, der nicht in diesem
  Repository liegt, und wird abgelehnt, wenn es nicht passt. Abschalten lässt
  sich das nicht.

  Nicht jede Installation kann sich selbst ersetzen: ein Debian-Paket unter
  `/usr` braucht Root. Dort sagt der Dialog das und bietet die Release-Seite
  an, statt auf halbem Weg zu scheitern.
- **Tooltips.** Jeder Knopf ohne Beschriftung erklärt sich, wenn der Zeiger
  einen Moment darauf steht. Eigentlich sollte er das längst — die Texte waren
  alle geschrieben —, aber der eingebaute Tooltip zeigt in der WebView dieses
  Programms gar nichts, die Texte waren also Dekoration. Dieser wird vom
  Programm selbst gezeichnet und sieht auf allen drei Systemen gleich aus.
- **Auf Anfang**, unter Darstellung: Größen, Seiten, Farben, Sprache und was
  ausgeblendet ist, in einem Knopf. Er fragt zweimal. Server,
  Übertragungseinstellungen und Warteschlange bleiben unberührt — das ist ein
  Zurücksetzen der Möbel, nicht der Arbeit.
- **Die Version steht in der Titelleiste.** Vorher stand sie in keinem Fenster.
- **Server-Log und Warteschlange lassen sich ausschalten**, in derselben Zeile,
  die sagt, wohin sie gehören.

### Geändert

- **Die Adresszeile ist ein Eingabefeld.** Ein Pfad, den man schon hat — aus
  einer Mail, aus einem Terminal —, lässt sich hineinkopieren. Während des
  Tippens passiert nichts: auf jedem Tastendruck zu navigieren liefe auf dem
  Weg nach irgendwo erst nach „/v" und „/va", und auf einem entfernten Server
  sind das drei Anfragen, die niemand wollte.
- **Der Ordnerbaum lässt sich breiter ziehen**, mit demselben Splitter wie
  zwischen den Seiten. Die Breite wird pro Seite gemerkt: ein tiefer Baum auf
  dem Server und ein flacher lokal wollen zwei verschiedene Breiten.
- **Die Darstellungseinstellungen öffnen als Dialog**, statt sich als zwei
  Knopfreihen unter der Fußzeile aufzuklappen, wo vier Gruppen ohne jede
  Trennung nebeneinander standen.
- **Der Server-Knopf leuchtet nicht mehr.** Ein umrandeter Akzent-Knopf oben im
  Fenster sitzt den ganzen Tag im Augenwinkel. Der Akzent steckt jetzt im
  Symbol, nicht im Knopf.
- Der Update-Knopf bietet „Update auf 0.1.5" an, statt „Fassung 0.1.5"
  festzustellen, und trägt einen Pfeil statt eines Sterns — ein Stern markiert
  einen Favoriten, hier geht es um etwas Neueres.
- Ein Klick darauf tut jetzt sichtbar etwas, wenn dort schon „Aktuell" steht.
  Die Antwort kommt nach vierzig Millisekunden und das Wort ändert sich nicht,
  das sah aus wie ein toter Knopf.

## 0.1.3

Der Weg zu einem gespeicherten Server, der bisher voraussetzte, dass man weiß,
wo man suchen muss.

### Geändert

- **Verbinden bietet die Server an, die du hast.** Bisher öffnete der Knopf ein
  Formular zum Eintippen einer Adresse — das Einzige, was jemand mit einem
  gespeicherten Server gerade nicht will. Die eigenen Server waren vom Knopf,
  dessen ganzer Zweck das Verbinden ist, nicht erreichbar. Jetzt öffnet er ein
  Menü: die gespeicherten Server zuerst, nach Ordner gruppiert, darunter
  Schnellverbindung und Serverliste. Taste und Knopf öffnen dasselbe Menü an
  derselben Stelle.
- **Die Serverliste hat einen Knopf, wo ein Hauptknopf hingehört** — in der
  Kopfzeile jeder Seite und beschriftet in einer neuen Leiste oben. Bisher saß
  er in der Fußzeile zwischen Darstellung und Hilfe, zwischen lauter Dingen,
  die niemand zweimal die Woche öffnet. Darstellung, Einstellungen und Tasten
  wandern mit nach oben, nach rechts.
- **Das Passwortfeld in den Servereinstellungen ist immer da.** Es erschien
  erst, wenn „merken" angehakt war, und las sich damit wie ein Fehler: Ein
  Feld, das unangekündigt auftaucht, erklärt nicht, warum es vorher nicht da
  war. Der Haken entscheidet jetzt, wie lange ein Passwort gilt, nicht ob man
  überhaupt eines angeben kann.

  Ohne Haken bleibt es für diesen Programmlauf und keinen Moment länger — im
  Arbeitsspeicher, nirgends abgelegt — und das Feld sagt, welche der beiden
  Zusagen gerade gilt. Dauerhaft speichern löscht die Kopie, die nur für diesen
  Lauf gedacht war.
- **Eine einmalige Verbindung in die Serverliste zu übernehmen** war ein Stern
  mit Tooltip. Niemand fährt über ein Symbol, um herauszufinden, ob es das
  gesuchte ist — jetzt steht dran, was es tut.

### Neu

- **Die Update-Prüfung sagt, woran sie ist.** Bisher meldete sie sich nur bei
  Neuigkeiten, Stille bedeutete also dreierlei: noch nicht gefragt, nichts
  Neues, oder GitHub nicht erreichbar. „Aktuell" zu behaupten, weil die Abfrage
  scheiterte, ist eine Vermutung im Gewand einer Antwort — deshalb ist
  „Prüfung fehlgeschlagen" ein eigener Zustand, und ein Klick versucht es
  erneut. Der Klick fragt auch dann, wenn die automatische Prüfung beim Start
  abgeschaltet ist: Diese Einstellung regelt das ungefragte Nachsehen, und ein
  Klick ist nicht ungefragt.

### Behoben

- Der Hinweis unter dem Passwort der Schnellverbindung nannte den
  Schlüsselbund, so heißt der Zugangsdatenspeicher aber nur auf dem Mac, und
  sprach vom „Site Manager", obwohl das Fenster in diesem Programm überall
  „Server" heißt.

## 0.1.2

Unter Windows und Linux war auch 0.1.1 unbenutzbar. Die dort gegebene Erklärung
war falsch; hier steht, woran es wirklich lag.

### Behoben

- **Das Serverfenster ging unter Windows weiterhin weiß auf.** Es wurde aus
  einem synchronen Befehl gebaut, wovor Tauris eigene Dokumentation warnt:
  Unter Windows blockiert das, weil WebView2 den Haupt-Thread braucht und der
  Befehl ihn bereits hält. Das Fenster erschien also, und darin begann nie
  etwas — in 0.1.0 eingefroren, in 0.1.1 weiß.

  Das Fragezeichen im Pfad, dem 0.1.1 das anlastete, war ein echter Fehler und
  richtig behoben, aber nicht dieser.
- **Außerhalb von macOS war jedes Tastenkürzel unerreichbar.** Sie lagen auf
  Meta — auf dem Mac die Befehlstaste, unter Windows die Windows-Taste, unter
  Linux Super. Tasten also, die sich das System nimmt, bevor ein Programm sie
  sieht. In der Statuszeile stand „⌘S" auf einem Rechner, der diese Taste gar
  nicht hat.

  Die Tabelle legt sich nicht mehr fest, welche physische Taste die Befehlstaste
  ist: auf dem Mac ⌘, sonst Strg, entschieden beim Start. Kürzel stehen in den
  Worten der Tastatur, die vor dir liegt, statt in Mac-Symbolen, und die beiden
  Belegungen, die es außerhalb des Macs nicht geben konnte — Vollbild und
  Löschen mit Rückschritt —, haben dort eine Antwort, die funktioniert.

  Selbst gewählte Tasten werden unverändert übernommen.
- **Der Dialog beim ersten Start erklärte Windows-Nutzern Mission Control.** Er
  existiert für ein einziges Problem — auf dem Mac sind F1 bis F12 keine
  Funktionstasten — und erschien trotzdem überall, bis hin zu Knöpfen, die die
  macOS-Systemeinstellungen öffnen wollten. Er kommt jetzt nur noch dort, wo es
  dieses Problem gibt. Sonst startet eine neue Installation direkt mit der
  Belegung der älteren Windows-Programme, und dafür ist dieses Programm da.

### Neu

- Ein Fenster, das nicht starten kann, sagt das. Jeder Fehler vor oder während
  des Starts wird in die Seite geschrieben, und wenn nach acht Sekunden nichts
  gezeichnet wurde, sagt sie auch das. Sein Schweigen hat den Fehler oben
  überhaupt erst eingekreist: Eine Seite, die nie startet, kann nichts melden —
  auch das nicht.

### Geändert

- Die Tastatur-Seite sagt unmissverständlich, dass der macOS-Teil für Windows
  und Linux nicht gilt, und die ausgeschriebenen Kürzel auf den anderen Seiten
  nennen beide Tastaturen.
- Das Wächter-Skript prüft die Tastatur gegen beide Tastaturen statt nur gegen
  die des Rechners, auf dem es läuft. Durch diese Lücke ist das überhaupt
  ausgeliefert worden.

## 0.1.1

Unter Windows war 0.1.0 unbenutzbar, und das ist der Grund.

### Behoben

- **Das Serverfenster ging leer auf, fror ein und riss den Rest mit.** Einem
  Fenster wird ein *Pfad* zum Laden gegeben, und der Pfad lautete
  `index.html?view=sites`. Ein Fragezeichen ist in einem Pfad unter macOS ein
  gewöhnliches Zeichen und unter Windows ein verbotenes — das Fenster hatte
  also nichts zu laden. Welche Ansicht ein Fenster zeigt, entscheidet jetzt
  seine Beschriftung.

  Dieser eine Fehler hat alle drei gemeldeten Erscheinungen verursacht. Die
  Antwort jedes Befehls läuft über denselben Faden zurück ins Fenster, den das
  eingefrorene Fenster blockiert hatte — deshalb blieb der Einstellungsdialog
  leer, und „verbindet …" stand still, obwohl die Verbindung längst stand.
- **Verbinden konnte ewig warten**, und das war nie ein Windows-Problem. Ein
  Server, der die Verbindung annimmt und dann schweigt — eine Firewall, die
  verschluckt statt abzulehnen; ein Port, hinter dem ein anderes Programm sitzt
  — wurde abgewartet, bis jemand aufgab. Zwanzig Sekunden decken jetzt das
  ganze Hallo ab, und die Meldung sagt, dass da *etwas* geantwortet hat: genau
  das schickt dich zur Firewall statt zur Adresse.
- **Serverliste und Importsuche** laufen nicht mehr auf dem Faden, der zeichnet.
  Die eine fragt den Zugangsdatenspeicher je Eintrag, die andere läuft
  Downloads, Schreibtisch und Dokumente zwei Ebenen tief ab.

### Geändert

- Das macOS-Abbild verlangt beim Öffnen keine Zustimmung zur ganzen AGPL mehr.
  Die Lizenz regelt die Weitergabe, nicht die Benutzung.
- Das Linux-Paket legt sein 256-Pixel-Symbol unter `256x256` ab statt unter
  `256x256@2` — ein Verzeichnisname, den keine Oberfläche kennt.

## 0.1.0

Der erste öffentliche Stand. Alles unten funktioniert und ist benutzt worden;
was fehlt, steht am Ende.

### Übertragen

- **SFTP, FTPS und FTP.** Explizites `AUTH TLS` als Voreinstellung, implizit auf
  Port 990, und reines FTP, wo ein Server nichts anderes anbietet — rot
  gekennzeichnet, solange es offen ist.
- **Mehrere Dateien gleichzeitig**, pro Server einstellbar. Acht über SFTP, vier
  über FTP — und lehnt ein Server eine weitere Anmeldung ab, senkt AmberBeam den
  Wert selbst und sagt es, statt den Rest der Warteschlange in Fehlern enden zu
  lassen.
- **Eine Warteschlange, die den Neustart übersteht.** Programm mitten in einer
  Übertragung schließen — der Rest ist beim nächsten Start noch da.
- **Abgerissene Übertragungen machen weiter**, wo sie aufgehört haben — aber nur,
  wenn das andere Ende noch dieselbe Größe und dasselbe Alter hat. Hat es sich
  geändert, wird neu begonnen, statt zwei Fassungen zusammenzunähen.
- **Bei Konflikten wird gefragt**, mit „für die übrigen genauso" im selben
  Dialog.
- Dateien zwischen den Bereichen ziehen, oder aus dem Finder herein.

### Server

- **Eine Serverliste in Ordnern**, eine lesbare JSON-Datei je Eintrag, und in
  keiner davon ein Passwort: die gehen in den Schlüsselbund, die
  Anmeldeinformationsverwaltung oder den Secret Service.
- **Import aus fünf anderen Programmen**: FileZilla, WinSCP, Total Commander,
  OpenSSH sowie `Sites.dat` oder der `.ftp`-Export eines älteren
  Windows-Clients. Ordner kommen mit.
- **Export** als reines JSON ohne Passwörter, oder mit ihnen unter einem
  Kennwort verschlossen. Ein Drittes gibt es nicht.
- Schlüsseldateien einschließlich PuTTYs `.ppk`, beide Fassungen, verschlüsselt
  oder nicht.

### Prüfen, wer am anderen Ende ist

- **SSH-Host-Keys** gegen `~/.ssh/known_hosts`. Ein geänderter Schlüssel wird
  abgewiesen.
- **TLS-Zertifikate** gegen den Vertrauensspeicher des Systems. Was nicht stimmt,
  wird beim Namen genannt — selbstsigniert, abgelaufen, noch nicht gültig,
  falscher Name, zurückgezogen, Signatur kaputt — statt zu „Zertifikatsfehler"
  zusammengefasst. Annehmen gilt für ein Zertifikat auf einem Host und Port.
- FTPS verschlüsselt auch den Datenkanal, oder die Verbindung scheitert und sagt,
  welche Hälfte der Server verweigert hat.

### Bedienung

- Drei Tastaturbelegungen, jede Taste änderbar, und ein Dialog beim ersten
  Start, der herausfindet, welche der F-Tasten-Hürden auf dem Mac bei dir im Weg
  steht — indem er dich eine Taste drücken lässt.
- Liste beim Tippen filtern; Unterordner auf Verlangen durchsuchen, mit
  hinterher ausgewiesenen Kosten.
- Raw-FTP-Befehle unter dem Log.
- Hell, dunkel und nach Systemvorgabe; fünf Akzentfarben; Deutsch und Englisch.

### Noch nicht dabei

- Kein FXP: Übertragungen zwischen zwei Servern laufen noch über diesen Rechner.
- Kein Verzeichnisvergleich, kein Spiegeln.
- Kein entferntes Bearbeiten.
- Keine zeitgesteuerten Aufträge.
- Die Container-Fassung mit Weboberfläche kommt noch.
- Nur macOS ist wirklich im Einsatz. Unter Linux ist das Debian-Paket in ein
  frisches Debian 12 installiert und geprüft worden: Abhängigkeiten stimmen,
  nichts fehlt, und Programm, Starteintrag und Symbole landen dort, wo sie
  hingehören — aber das Fenster hat noch niemand auf einem echten Linux-Rechner
  aufgehen sehen. Windows baut die CI, sonst ist es ungeprüft.
