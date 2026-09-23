# woodpecker

woodpecker lässt Menschen und Agenten ein Spiel kontrolliert ausführen,
beobachten und auf Fehler untersuchen.

## Sprache

**Session**: Eine unabhängig steuerbare Spielinstanz mit eigenen Eingaben,
Ausführungsschritten und Beobachtungen.

**Tick**: Ein ausdrücklich ausgeführter Simulationsschritt.

**Warp**: Ein Auftrag, eine bestimmte Anzahl Ticks auszuführen.

**Bildziel**: Die für den Agenten bestimmte Ansicht des Spiels mit eindeutigen
Abmessungen und einem Koordinatenraum für bildbezogene Eingaben.
Nicht mit einem Betriebssystemfenster gleichsetzen.

**Capture**: Eine angeforderte Bildaufnahme des Bildziels.
Ein Capture ist kein Simulationsschritt.

**Virtuelle Eingabe**: Eine der Session zugeordnete Tastatur-, Pointer- oder
Texteingabe des steuernden Menschen oder Agenten.

**UI-Fokus**: Das aktuell für Texteingabe ausgewählte Spielelement.
Nicht mit dem Fokus eines Betriebssystemfensters gleichsetzen.
