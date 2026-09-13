# Die Tastatur

[English](keyboard.md)

AmberBeam ist auf Tastaturbedienung gebaut. **F1** im Programm zeigt die
Belegung, wie sie gerade eingestellt ist — dieses Fenster liest dieselbe
Tabelle, aus der auch die Tasten kommen, kann also nicht veralten.

## Unter Windows und Linux

Nichts davon gilt hier. F1 bis F12 sind Funktionstasten, kein Teil des Systems
wartet dahinter, und AmberBeam startet gleich mit der Belegung der älteren
Windows-Programme. Beim ersten Start kommt kein Dialog, und es ist nichts
abzuschalten. Die Befehlstaste ist in allen Kürzeln **Strg**.

Der Rest dieser Seite betrifft macOS.

## Das F-Tasten-Problem auf dem Mac

Auf dem Mac liegen auf F1 bis F12 Helligkeit und Lautstärke. Zwei getrennte
Dinge stehen zwischen ihnen und einem Programm:

1. **Ein bloßes F5 ist Helligkeit.** `fn` + F5 kommt trotzdem an, sofort und
   ohne jede Umstellung. Eine Einstellung unter Systemeinstellungen ▸ Tastatur
   erspart das `fn` — eine Bequemlichkeit, keine Bedingung.
2. **F3, F4 und F11 gehören dem System.** Mission Control, Spotlight und
   „Schreibtisch zeigen" nehmen sie sich, bevor ein Programm sie sieht, egal
   was man hält. Jede lässt sich einzeln unter Tastaturkurzbefehle abschalten
   — oder AmberBeam nimmt für diese drei einfach andere Tasten.

Der Dialog beim ersten Start findet heraus, was bei dir im Weg steht — indem
er dich eine Taste drücken lässt und schaut, ob sie ankommt — und bietet drei
Belegungen an.

## Die drei Belegungen

| | Was es ist | Was es braucht |
|---|---|---|
| **Funktionstasten** | F5, F6, F8 und der Rest, so belegt wie in den älteren Windows-Programmen | F3, F4 und F11 in den Systemeinstellungen freigeben |
| **Gemischt** | Funktionstasten, wo das System sie lässt, ⌘ für die drei anderen | Nichts |
| **Ohne Funktionstasten** | Keine einzige, alles liegt auf ⌘ | Nichts |

Benannt nach dem, was sie tun, statt nach einem System — zwei der drei Namen
waren vorher Betriebssysteme, und keines davon war das, vor dem ein
Linux-Benutzer saß.

Alle drei gibt es auch unter Windows und Linux, unter **Tasten**, wo ⌘ sich als
Strg liest und wo keine davon eine Systemeinstellung braucht: F1 bis F12 sind
dort Funktionstasten, und nichts wartet dahinter. Einen Grund zu wählen hat
dort nur die erste; die beiden anderen bleiben, damit eine vom Mac
mitgebrachte Belegung weiter funktioniert. Das Fenster **Tasten** sagt, auf
welchem System du gerade bist.

Jede Taste jeder Belegung lässt sich ändern: **Tasten ▸ Tasten ändern**, Zeile
anklicken, gewünschte Taste drücken. Eine weggenommene Taste sagt, wem sie
gehörte. Eine Belegung lässt sich in eine Datei schreiben und auf einem
anderen Rechner wieder einlesen.

## Was nie umbelegt wird

Sich in einer Liste zu bewegen ist keine Geschmacksfrage:

| | |
|---|---|
| ↑ ↓ | Durch die Liste bewegen |
| ↵ | Ordner öffnen, Datei betreten |
| ⌫ | Einen Ordner aufwärts |
| Leertaste | Markieren und weiter |
| ⇞ ⇟ ⇱ ⇲ | Seitenweise, und an die Enden |

Das ist, was eine Liste ausmacht. Eine Belegung, die das änderte, wäre eine,
die niemand benutzen kann.

## Warum AmberBeam die Tasten nicht einfach abfängt

Es ginge. Tasten unterhalb des Betriebssystems abzufangen ist möglich — es
verlangt die Bedienungshilfen-Berechtigung, bricht bei jedem macOS-Update und
lässt ein Programm wie einen Tastaturmitschneider aussehen. Der Preis wäre
höher als der Gewinn.
