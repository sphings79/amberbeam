# Zwei Seiten vergleichen, eine beobachten

[English](comparing.md)

Zwei verschiedene Aufgaben auf dieselbe Frage — *was ist drüben nicht wie
hier* — und mit Absicht nicht dasselbe Werkzeug.

**Vergleichen** drückst du. Es liest beide Seiten, sagt, was verschieden ist,
und gibt dir die Liste.

**Beobachten** lässt du laufen. Es merkt, was sich auf diesem Rechner ändert,
und schickt es von selbst hinauf.

## Vergleichen

Der Knopf **Vergleichen** liest die beiden Seiten, wie sie gerade stehen. Vor
allem anderen fragt er drei Dinge.

**In welche Richtung.** Dateien reisen von links im Fenster nach rechts, und
der Knopf dazwischen dreht das um. Nichts entscheidet sich daran, welche Seite
du zuletzt angeklickt hast.

**Ob es bis nach unten geht.** Aus vergleicht es die zwei offenen
Verzeichnisse. An läuft es beide Bäume bis unten durch — das ist eine
Auflistung pro Verzeichnis auf jeder Seite, und auf einem Server ist jede
Auflistung ein Hin und Zurück. Ein Projekt mit einem `node_modules` darin sind
Tausende. Deshalb ist es eine Frage und keine Annahme.

**Was als gleich gilt.**

| | |
|---|---|
| **Größe** | Am billigsten, und blind für jede Änderung, die die Länge behält. |
| **Größe und Zeit** | Die übliche Antwort, und die, mit der es anfängt. |
| **Größe und Inhalt** | Liest beide Dateien vollständig. Sicher — und über einen Server kostet es so viel wie das Übertragen. |

Zwei Zeitstempel innerhalb von **zwei Sekunden** gelten als ein Moment. FTP
meldet Zeiten bestenfalls sekundengenau, viele Server runden auf Minuten, und
eine übertragene Datei kommt einen Moment später an, als sie gelesen wurde —
ohne dieses Fenster gälte jede je kopierte Datei als geändert.

Eine Seite, die **gar keine Zeit** meldet, kann über keine uneinig sein — das
gilt also als gleich. Sonst stünde bei einem Server, dessen Listing keine Daten
kennt, jedes Mal das ganze Projekt in der Warteschlange. Ist das dein Server,
nimm Prüfsummen; die ignorieren die Uhren vollständig, und genau dafür sind
sie da.

### Was zurückkommt

Jeder Name von beiden Seiten, mit dem, was über ihn herausgefunden wurde:

| | |
|---|---|
| **nur hier** | Auf der Seite, von der aus übertragen würde. Angehakt. |
| **verschieden** | Auf beiden, und nicht gleich. Angehakt. |
| **nur dort** | Nur auf der anderen Seite. Wird gezeigt und in Ruhe gelassen — außer Löschen wird mitgezogen, siehe unten. |
| **gleich** | Wird gezeigt, tritt zurück, ist nicht anhakbar. |

Gleiche Zeilen stehen mit Absicht da. Eine Liste aus lauter Unterschieden
lässt sich nicht mit dem abgleichen, was tatsächlich da ist.

Der Knopf legt das Angehakte **angehalten** in die Warteschlange — ein
Vergleich kann sich als vierhundert Dateien herausstellen, und das im selben
Moment loszuschicken ist keine Entscheidung, die jemand getroffen hat. Starte
es, wenn du es angesehen hast.

Ein angehaktes **Verzeichnis** nimmt alles darunter mit; dessen eigene Zeilen
werden nicht ein zweites Mal eingereiht.

Hält der Lauf bei fünftausend Verzeichnissen an, sagt er es ausdrücklich. Was
in einer stillschweigend unvollständigen Liste fehlt, sieht genau aus wie
Übereinstimmung — und das ist die gefährliche Art, falsch zu liegen.

## Beobachten

Der Knopf **Beobachten** beobachtet die lokale Seite und schickt, was sich
darin ändert, zur anderen. Solange das läuft, steht über den Seiten eine
Leiste: welches Verzeichnis, wohin, wie viele Dateien bisher — und ein Druck
hält es an. Was Dateien hochlädt, sobald sie sich ändern, darf nicht unsichtbar
sein.

**Eine Richtung, und nur eine ist möglich.** Das Betriebssystem sagt uns in dem
Moment Bescheid, in dem eine lokale Datei geschrieben wird. Kein Server kann
dergleichen — weder FTP noch SFTP hat eine Möglichkeit, eine Änderung zu
melden —, die andere Seite zu beobachten hieße also, sie immer wieder zu
fragen: eine Dauerlast auf der Maschine eines anderen statt eines
Hintergrunddienstes. Der Knopf sagt das, statt beim Drücken zu scheitern.

**Nach dem Überschreiben wird nicht gefragt.** Während das läuft, schaut
niemand hin, und eine Frage würde die Warteschlange auf eine Antwort warten
lassen, die nicht kommt. Die lokale Datei ist die, für deren Transport die
Beobachtung da ist.

Ereignisse kommen in Schüben — ein Editor schreibt, benennt um und fasst
nochmal an, alles in Millisekunden —, also wird gesammelt und einmal gehandelt,
sobald es ruhig ist.

Eine Beobachtung lebt so lange wie das Programm. Im Container lebt sie so lange
wie der Dienst und überdauert damit den Browsertab, der sie gestartet hat; die
Leiste fragt nach, was läuft, statt nur zu kennen, was sie selbst begonnen hat.

## Was nie angesehen wird

**Einstellungen → Bearbeiten** ist fürs Bearbeiten. Dies ist eine andere
Liste: Muster am **Servereintrag**, unter *Nie ansehen*. `*` steht für
beliebiges, Groß- und Kleinschreibung ist egal, und ein Name greift auf jeder
Ebene — eine Regel für `node_modules` nimmt alles darin aus.

Sie gehört zum Eintrag, weil auf jedem Server etwas anderes Lärm ist. Das
Vergleichsfenster füllt sich daraus und lässt sie für einen Durchgang ändern,
ohne den Eintrag zu ändern.

## Die zwei Schalter

**Einstellungen → Vergleichen und Beobachten**:

**Vor dem Übertragen zeigen, was gefunden wurde** — an. Aus geht alles
Unterschiedliche direkt in die Warteschlange, weiterhin angehalten.

**Löschen mitziehen** — aus, und das sollte es bleiben. An heißt: Eine Datei,
die hier verschwindet, verschwindet auch dort — die Beobachtung entfernt sie,
und im Vergleich werden Einträge, die es nur drüben gibt, anhakbar, wobei der
Knopf sie getrennt von den Übertragungen zählt. Sonst nimmt ein Checkout, ein
aufräumender Build oder ein verrutschtes Verschieben Dateien vom Server, und du
erfährst es vom Server.

Eine Beobachtung liest diesen Schalter einmal, beim Start. Änderte er sich
unter einer laufenden Beobachtung, könnte hinterher niemand sagen, was sie
getan hat.

Löschungen gehen nicht durch die Warteschlange — die trägt eine Datei von hier
nach dort, und etwas wegzunehmen ist das nicht. Sie passieren zuerst, damit ein
drüben frei gewordener Name im selben Durchgang neu belegt werden kann. Ist für
den Server ein Papierkorb eingetragen, landen sie darin statt im Nichts.

Dafür muss der Server eine Datei umbenennen, und viele FTP-Server verweigern
das grundsätzlich — der Testserver dieses Projekts zum Beispiel. Dann wird
nichts gelöscht, der Papierkorb wird für diesen Eintrag abgeschaltet, und eine
Beobachtung sagt, wie viele Änderungen sie nicht ausführen konnte. Die Datei
liegt weiter oben, und genau so soll das schiefgehen.

Siehe auch: [Eine Datei dort bearbeiten, wo sie liegt](editing.de.md).
