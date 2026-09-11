# Erste Schritte

[English](getting-started.md)

## Zwei Bereiche, und jeder kann überall sein

Beide Seiten fangen auf diesem Rechner an. Verbindest du eine mit einem
Server, hast du die gewohnte Anordnung; verbindest du beide, stehen zwei
Server nebeneinander. Nichts in AmberBeam nimmt an, dass eine Seite „die
lokale" ist — das war vor der ersten Zeile Code entschieden, und deshalb ist
Übertragung zwischen zwei Servern eine Frage des Wann, nicht des Ob.

**Tab** oder **F6** wechselt die Seite. Die mit dem helleren Rahmen ist die,
mit der die Tasten sprechen.

## Verbinden

Der **+**-Knopf am Bereich, oder **F12**, öffnet den Verbindungsdialog. Zuerst
das Protokoll — der Port folgt ihm, solange du keinen selbst getippt hast:

| | Port | |
|---|---|---|
| **SFTP** | 22 | Über SSH. Nimm das, wenn der Server es anbietet. |
| **FTPS (explizit)** | 21 | Gewöhnliches FTP, das mit `AUTH TLS` auf TLS umschaltet. |
| **FTPS (implizit)** | 990 | TLS ab dem ersten Byte. Älter und seltener. |
| **FTP** | 21 | Gar keine Verschlüsselung. Rot gekennzeichnet, solange es offen ist. |

Ein Server, den du wieder brauchst, gehört in die Serverliste: **⌘S** oder der
Knopf *Server*. Einträge dort behalten alles — wo beide Seiten beginnen, wie
viele Übertragungen gleichzeitig, eine Farbe zum Auseinanderhalten — und ihre
Passwörter gehen in den Zugangsdatenspeicher dieses Rechners, nicht in eine
Datei.

## Dateien bewegen

Mit der **Leertaste** oder der Maus markieren, dann:

- Der Übertragen-Knopf in der Werkzeugleiste, oder die Auswahl in den anderen
  Bereich ziehen
- Dateien aus dem Finder hineinziehen
- Ein Ordner geht mit allem darin, und die Struktur bleibt erhalten

Alles landet in der **Warteschlange** unten. Sie übersteht einen Neustart:
Schließt du das Programm mitten in einer Übertragung, ist der Rest beim
nächsten Start noch da — und eine halb übertragene Datei macht dort weiter, wo
sie aufgehört hat, statt von vorn zu beginnen.

## Wenn schon etwas da ist

AmberBeam fragt, statt zu entscheiden. Überschreiben, überspringen, beides
behalten, oder erst Größe und Datum vergleichen — und „für die übrigen genauso"
steht im selben Dialog, weil aus einer Warteschlange mit zweihundert Dateien
nicht zweihundert Rückfragen werden dürfen.

## Wenn eine Übertragung abreißt

Sie macht weiter, wo sie aufgehört hat — aber nur, wenn das sicher ist: Die
Datei am anderen Ende muss noch dieselbe Größe und dasselbe Alter haben wie
beim Abbruch. Hat sie sich geändert, fängt AmberBeam neu an, statt zwei
Fassungen zusammenzunähen. Eine aus zwei Fassungen zusammengesetzte Datei
sieht vollständig aus und ist es nicht — das ist der eine Fehler, den dieses
Programm nie erzeugen soll.

Manche FTP-Server können eine Übertragung gar nicht fortsetzen. Die sagen das
beim Verbinden, nicht erst, wenn etwas abreißt.
