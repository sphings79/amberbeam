# Eine Datei dort bearbeiten, wo sie liegt

[English](editing.md)

Rechtsklick auf eine Datei auf dem Server, **Remote bearbeiten**. Eine Kopie
kommt auf diesen Rechner, irgendetwas öffnet sie, und jedes Speichern geht
direkt wieder hinauf.

Das ist die ganze Idee. Alles Weitere steht hier, weil es die Arten gibt, auf
die das leise schiefgeht.

## Womit sie aufgeht

**Einstellungen → Bearbeiten** enthält eine Tabelle: Dateiarten, und was jede
davon öffnet.

| | |
|---|---|
| **AmberBeam** | Der eingebaute Editor. Zeilennummern, Suche, und ein Speichern, das heißt, dass sich der Server geändert hat. |
| **Was das System nimmt** | Womit dieser Rechner diese Dateiart öffnet. |
| **Ein Programm…** | Eines, das du angibst. Auf dem Mac geht ein Programmbündel genauso wie eine ausführbare Datei. |

Die erste passende Zeile gewinnt — eine oben hinzugefügte schlägt also die
lange darunter, ohne dass man sie dort herausschneiden muss. Eine Zeile kann
eine Endung nennen (`php`) oder einen ganzen Dateinamen für die ohne Endung:
`dockerfile` und `makefile` stehen drin, und `.htaccess` zählt als `htaccess`.

**Was nicht in der Tabelle steht, wird nicht zum Bearbeiten angeboten.** Das
ist dieselbe Entscheidung einmal statt zweimal gesagt: Eine getrennte Liste
„was gilt als Text" wäre sich mit dieser irgendwann über dieselbe Datei
uneinig.

Darunter liegt noch eine Prüfung, die sich nicht abschalten lässt: Eine Datei
mit einem Nullbyte in den ersten Kilobytes ist kein Text, wie sie auch heißt,
und wird abgelehnt. Ein PNG, das jemand `logo.php` genannt hat, ist weiterhin
ein PNG.

## Speichern

Der eingebaute Editor speichert mit **⌘S**. Ein fremdes Programm speichert, wie
es eben speichert — AmberBeam sieht sich die Kopie einmal pro Sekunde an,
solange sie offen ist, und schickt hinauf, was es vorfindet. Anders geht es
nicht: Ein Programm sagt niemandem, dass es eine Datei geöffnet hat, und beim
Schließen auch nicht.

Ob etwas hochgegangen ist, steht im Server-Log, bei allem anderen, was diesem
Server widerfahren ist.

## Wenn jemand anderes in der Datei war

Wie die Datei aussah, als die Kopie genommen wurde, wird gemerkt. Vor dem
Zurückschreiben sieht AmberBeam noch einmal nach — und ist es nicht mehr
dieselbe Datei, wird **nichts geschrieben** und du wirst gefragt.

Die Größe immer, der Zeitstempel nur, wenn beide Blicke einen genannt haben.
Viele FTP-Server antworten auf `MDTM` mit nichts Brauchbarem, und eine Frage,
die niemand beantworten kann, ist schlimmer als keine.

So oder so liegt das Getippte zuerst sicher in der Kopie — die Arbeit ist nie
der Preis dafür, eine Frage darüber zu beantworten.

## Kodierungen und Zeilenenden

Aus den Bytes gelesen und genau so zurückgeschrieben, wie vorgefunden:

- **Kein gültiges UTF-8** wird als Latin-1 gelesen und als Latin-1
  zurückgeschrieben. So eine Datei als UTF-8 zu lesen und zu speichern
  schreibt jeden Umlaut darin neu, und es fällt erst viel später auf.
- **Ein Byte Order Mark**, das da war, bleibt da.
- **CR-LF-Zeilenenden** bleiben CR LF. Ein Editor, der das stillschweigend
  umstellt, macht aus jeder Zeile des nächsten Diffs eine Änderung.

Tippst du ein Zeichen, für das die Kodierung der Datei keinen Platz hat — ein
Emoji in eine Latin-1-Datei —, wird das Speichern mit Nennung des Zeichens
abgelehnt. Ein Fragezeichen dort, wo jemand ein Wort getippt hat, ist die
falsche Art von Hilfe.

## Die Kopien

Sie liegen in einem eigenen Verzeichnis unter dem temporären dieses Systems,
eines pro Datei, jede mit dem Namen, den sie hatte.

Den eingebauten Editor zu schließen beendet die Bearbeitung, und die Kopie geht
weg: Das Fenster zu schließen, in dem man getippt hat, ist die eine Art, wie
dieses Programm wissen kann, dass du fertig bist. Bei einer Datei in einem
fremden Programm gibt es diesen Moment nicht, also läuft die Beobachtung, bis
AmberBeam beendet wird — und dann wird gefragt, ob die Kopien weg sollen.

Kopien aus einem Lauf, der schlecht endete, werden beim nächsten Start
weggeworfen. Das Verzeichnis der offenen Bearbeitungen liegt im Speicher; was
dort noch herumsteht, gehört also niemandem.

**Sie sind nicht verschlüsselt.** Eine Kopie ist eine Datei von deinem Server
auf deiner Platte, solange sie offen ist — dasselbe wie bei jedem anderen
Download, aber man sollte es wissen, statt etwas anderes anzunehmen.

## Grenzen

- Dateien über 20 MB werden nicht angeboten. Der Text reist als eine einzige
  Zeichenkette durch einen Befehl, und ein 400-MB-Log will ein anderes
  Werkzeug.
- Dieselbe Datei zweimal zum Bearbeiten zu öffnen gibt dieselbe Kopie zurück.
  Zwei Kopien einer Datei, jede ohne Wissen von der anderen, ist ein Wettlauf
  mit deinem Nachmittag als Einsatz.
- **Im Container nur der eingebaute Editor.** Der Dienst läuft auf der fernen
  Maschine, und ein dort gestartetes Programm erschiene nicht vor dir. Die
  Datei geht stattdessen im Editor hier auf, und das Log sagt warum. Siehe
  [Als Container betreiben](container.de.md).

Siehe auch: [Wo die Geheimnisse liegen](security.de.md).
