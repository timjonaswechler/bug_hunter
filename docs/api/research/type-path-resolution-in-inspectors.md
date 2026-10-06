# Type-Path-Auflösung in bevy-inspector-egui, Bevy Remote und Jackdaw

## Fragestellung

Wie behandeln `bevy-inspector-egui` und Jackdaw vollständige, unbekannte oder nicht mehr
registrierte Type Paths, und welche Regel lässt sich daraus für `command::inspect::query` ableiten?

Untersucht wurden:

- `bevy-inspector-egui` am bereits für Bevy 0.19 geprüften Commit
  [`58782ac`](https://github.com/jakobhellermann/bevy-inspector-egui/commit/58782ac050dddd64527739215b50169ef2da83a4),
- Bevy Remote Protocol 0.19.1,
- Jackdaw am Commit
  [`35d0411`](https://github.com/jbuehler23/jackdaw/commit/35d0411c4afd9ab72f00b40ae3b44434fb8645a0).

## bevy-inspector-egui

`bevy-inspector-egui` besitzt für den allgemeinen World-Inspector keine von außen gelieferte
String-Query. Es entdeckt die vorhandenen Typen selbst:

- Entity-Komponenten werden aus dem Archetype als `ComponentId` und optionaler `TypeId` gelesen.
  Ein fehlender `TypeId` oder fehlende Reflection-Daten werden im Inspector als Fehler angezeigt,
  statt einen Type-Path-String aufzulösen. Siehe
  [`components_of_entity`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/bevy_inspector/mod.rs#L731-L750)
  und die Verarbeitung in
  [`ui_for_entity_components`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/bevy_inspector/mod.rs#L575-L629).
- Ressourcen werden aus der `TypeRegistry` gesammelt, auf `ReflectResource` gefiltert und danach
  über `TypeId` verarbeitet. Der kurze Type Path dient nur als UI-Name. Siehe
  [`ui_for_resources`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/bevy_inspector/mod.rs#L103-L126).
- Assets werden ebenso aus der Registry gesammelt, auf `ReflectAsset` gefiltert und über `TypeId`
  verarbeitet. Siehe
  [`ui_for_all_assets`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/bevy_inspector/mod.rs#L155-L178)
  und
  [`by_type_id::ui_for_assets`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/bevy_inspector/mod.rs#L905-L935).

Damit vermeidet `bevy-inspector-egui` die offene Frage für normale UI-Nutzung. Der Nutzer wählt aus
entdeckten Werten. Ein unbekannter Type Path kann nicht als freie Eingabe in die Query gelangen.
Fehlende Registry- oder Reflection-Daten bleiben am entdeckten Wert sichtbar.

## Bevy Remote Protocol 0.19.1

Bevys `world.query` akzeptiert vollständige Type Paths. Es besitzt mit `strict` ausdrücklich zwei
Verhaltensweisen:

- Mit `strict: true` führt ein nicht registrierter oder nicht im World verwendeter Component-Pfad zu
  einem Fehler.
- Mit dem Standard `strict: false` gilt:
  - unbekannt in erforderlichen `components` oder in `filter.with`: leere Response,
  - unbekannt in optionalen Components oder `filter.without`: ignorieren,
  - unbekannt in `has`: als nicht vorhanden, also `false`, ausgeben.

Die Regeln stehen direkt in
[`process_remote_query_request`](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_remote/src/builtin_methods.rs#L857-L916).
Die Auflösung verwendet vollständige Type Paths und sammelt bei nicht-strikter Ausführung nicht
aufgelöste Pfade getrennt. Siehe
[`get_component_ids`](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_remote/src/builtin_methods.rs#L1816-L1854).

Mutationen sind dagegen strikt. Ein unbekannter Component-Type-Path wird als Fehler abgelehnt. Siehe
[`process_remote_mutate_component_request`](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_remote/src/builtin_methods.rs#L1172-L1192).

## Jackdaw

Jackdaw verwendet für seine Live-Queries Bevy Remote `world.query`:

- `QuerySpec` speichert vollständige Reflect-Type-Paths und übergibt `with` und `without` direkt an
  BRP. Siehe
  [`QuerySpec`](https://github.com/jbuehler23/jackdaw/blob/35d0411c4afd9ab72f00b40ae3b44434fb8645a0/src/remote/debug/queries.rs#L23-L58).
- Jackdaw setzt kein `strict`-Feld. Deshalb gilt Bevys Standard `strict: false`.
- Die UI bietet Type Paths aus der Registry der verbundenen Anwendung an. Freie Type-Path-Eingaben
  sind nicht der normale Pfad. Siehe
  [`registry_component_options`](https://github.com/jbuehler23/jackdaw/blob/35d0411c4afd9ab72f00b40ae3b44434fb8645a0/src/remote/debug/queries.rs#L162-L190).
- Die gespeicherten Component-Definitionen sind nach vollständigem Type Path indiziert. Siehe
  [`ComponentsFile`](https://github.com/jbuehler23/jackdaw/blob/35d0411c4afd9ab72f00b40ae3b44434fb8645a0/crates/jackdaw_remote/src/schema.rs#L4-L12).
- Erhält der Remote Inspector einen Component-Wert, dessen Type Path lokal nicht registriert ist,
  verwirft Jackdaw ihn nicht. Es zeigt den Wert in einer Raw-JSON-Fallback-Darstellung. Dasselbe gilt
  bei fehlendem `ReflectComponent` oder fehlgeschlagener Deserialisierung. Siehe
  [`populate_proxy_from_components`](https://github.com/jbuehler23/jackdaw/blob/35d0411c4afd9ab72f00b40ae3b44434fb8645a0/src/remote/remote_inspector.rs#L161-L190).

Jackdaw kombiniert damit zwei Regeln: Auswahl aus der Registry verhindert im normalen UI ungültige
Pfade, und die zugrunde liegende read-only BRP-Query bleibt nicht strikt. Nicht lokal darstellbare
bereits gelieferte Werte bleiben als JSON sichtbar.

## Folgerung für bug_hunter

Die beiden Vergleichsprojekte sprechen nicht für eine pauschale Ablehnung unbekannter Pfade bei
read-only Queries:

- `bevy-inspector-egui` verhindert freie ungültige Eingaben durch Discovery.
- Jackdaw verwendet Bevys nicht-strikte Query-Semantik.
- Bevy selbst trennt tolerante Queries von strikt abgelehnten Mutationen.

Für `bug_hunter` passt daher folgende Regel am besten zu Bevy:

| Inspect-Eingabe | Unbekannter Type Path |
| --- | --- |
| Entity `with` | leere Ergebnismenge |
| Entity `without` | Pfad schließt keine Entity aus |
| `component::Selection::Listed` | Component-Ausgabe mit `Unavailable::NotRegistered` |
| Resource `Metadata` | leere Ergebnismenge |
| Resource `Value` | `Unavailable::NotRegistered` |
| Asset-Typ ohne konkreten Key | leere Ergebnismenge |
| Asset-Typ mit Key und `Value` | `Unavailable::NotRegistered` |
| `Set` | Command ablehnen, bevor eine Mutation stattfindet |

`bug_hunter` sollte vollständige Type Paths unverändert über Bevys `TypeRegistry` auflösen. Es
braucht keine eigene Short-Path-Auflösung und keine eigene Definition von Mehrdeutigkeit. Die
abweichenden Ausgabeformen ergeben sich nur daraus, dass `bug_hunter` im Gegensatz zu den UI-Tools
einen typisierten Wire-Vertrag besitzt.

## Nachgelagerte Projektentscheidung

Nach Auswertung dieser technischen Empfehlung wurde für `bug_hunter` bewusst eine strengere Regel
gewählt: Jeder vom Aufrufer gelieferte Type Path muss sich vor der Ausführung exakt über Bevys
`TypeRegistry` auflösen lassen. Andernfalls wird der gesamte Query-Command ohne Teilergebnis
abgelehnt. Inspect wurde anschließend vollständig auf read-only Entity- und Resource-Queries
begrenzt; die in der Tabelle mitbewerteten Asset- und `Set`-Fälle gehören nicht mehr zum
Zielvertrag. Damit bleibt die oben beschriebene tolerante Semantik ein Rechercheergebnis zu BRP und
Jackdaw, ist aber nicht der Zielvertrag von `bug_hunter`.
