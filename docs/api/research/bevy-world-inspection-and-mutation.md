# Research: Bevy-World-Inspection und Mutation

> **Nachgelagerte Projektentscheidung:** `command::inspect` wurde nach dieser technischen Prüfung
> vollständig auf read-only Entity- und Resource-Queries begrenzt. Allgemeine Asset-Inspection und
> die dokumentierten Mutationspfade bleiben technische Rechercheergebnisse, gehören aber nicht mehr
> zum Zielinterface von `bug_hunter`.

## Kurzfazit

Der Bevy-0.19-Kompatibilitätsstand von `bevy-inspector-egui` verwendet keinen JSON- oder Query-Parser. Der exakte UI-Pfad ist: `TypeRegistry`/`AppTypeRegistry` → `RestrictedWorldView` → untypisierte ECS-/Asset-Pointer → `ReflectFromPtr` beziehungsweise `ReflectAsset` → `InspectorUi`/`PartialReflect`-Mutation. Die Mutation des bereits gelesenen Werts ist direkt und synchron; `CommandQueue` hält nur nachgelagerte World-Kommandos zurück, bis die eingeschränkten Borrows beendet sind.

Das Verfahren ist für einen JSON-Inspektionsbefehl wiederverwendbar, aber die UI-Schicht selbst ist kein geeigneter Wire-Vertrag. Registrierung, reflektierte Typdaten, mutierbare ECS-Werte, Asset-Storage und eine explizite Regel für nicht serialisierbare beziehungsweise nicht deserialisierbare JSON-Werte sind Voraussetzungen. Die Aussagen unten unterscheiden bestätigte Quellbefunde von Design-Implikationen.

## Befunde

1. **[Info] Geprüfter Stand, Registrierungen und UI-Voraussetzungen** — Der geprüfte Commit [`58782ac050dddd64527739215b50169ef2da83a4`](https://github.com/jakobhellermann/bevy-inspector-egui/commit/58782ac050dddd64527739215b50169ef2da83a4) stellt in seinem `Cargo.toml` die Bevy-0.19-Abhängigkeiten ein. Die `v0.35.0`- und `v0.36.0`-Tags gehören dagegen zu älteren Bevy-Versionen; für eine Abhängigkeit muss daher dieser Commit oder ein späterer kompatibler Release gepinnt werden. **Severity: hoch – falsches Tag erzeugt eine Bevy-Abhängigkeitsinkompatibilität.**

   `WorldInspectorPlugin` in [`crates/bevy-inspector-egui/src/quick.rs`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/quick.rs) prüft `MainSchedulePlugin` und `EguiPlugin`, registriert bei Bedarf `DefaultInspectorConfigPlugin` und plant die UI in `EguiPrimaryContextPass`. Die UI erwartet eine Entität mit `PrimaryEguiContext`. Diese Voraussetzungen betreffen nur die UI; ein eigener JSON-Inspektor braucht sie nicht.

   `DefaultInspectorConfigPlugin` in [`crates/bevy-inspector-egui/src/lib.rs`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/lib.rs) registriert Standard-`InspectorEguiImpl`s und Typoptionen. Die Anwendung muss die eigentlichen Komponenten und Ressourcen weiterhin mit `App::register_type` beziehungsweise geeigneten `#[reflect(...)]`-Attributen registrieren. Im lokalen Crate verlangt [`crates/bug_hunter/Cargo.toml`](../../../Cargo.toml) Bevy 0.19; ein Inspector-Dependency ist dort derzeit nicht vorhanden.

2. **[Info] `RestrictedWorldView` ist die Borrow- und Zugriffsgrenze.** — [`crates/bevy-inspector-egui/src/restricted_world_view.rs`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/restricted_world_view.rs) hält eine `UnsafeWorldCell` sowie Allow-/Forbid-Listen für `TypeId`-Ressourcen und `(Entity, TypeId)`-Komponenten.

   - `RestrictedWorldView::new(&mut World)` erlaubt zunächst alles.
   - `split_off_resource`/`split_off_resource_typed` erzeugen eine View nur für eine Ressource und entfernen deren Zugriff aus der Rest-View.
   - `split_off_component` beziehungsweise `split_off_components` tun dasselbe für konkrete Entity-Komponenten.
   - `resources_components` erzeugt getrennte Resource- und Component-Views.
   - Jede sichere Zugriffsmethode prüft die Allow-Liste. Die internen `unsafe`-Pfade sind nur korrekt, wenn der aufgeteilte Zugriff und die Typ-ID-Gleichheit eingehalten werden.

   Der Inspector hält damit den gerade mutierten Wert in einer View und übergibt eine disjunkte Rest-World an verschachtelte Inspector-Operationen, beispielsweise für einen `Handle<T>`. **Design-Implikation:** Ein JSON-Backend sollte dieselbe Trennung beibehalten und nicht gleichzeitig einen untypisierten mutablen Pointer und eine überlappende `&mut World`-Sicht weiterreichen.

3. **[Info] Entity- und Komponentenfluss: Entdeckung, Lesen und Schreiben.** — `ui_for_entities_filtered` in [`crates/bevy-inspector-egui/src/bevy_inspector/mod.rs`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/bevy_inspector/mod.rs) verwendet für die primäre Entity-Menge eine statisch kompilierte `query_filtered::<Entity, F::StaticFilter>()`; die Textsuche läuft danach über Entity-Namen und Children. Die Default-Filter enthalten `Without<ChildOf>` und `Without<IsResource>`. Das ist kein runtime aus Type Paths gebauter Query.

   `ui_for_entity_components` entdeckt die Komponenten eines Entities in `components_of_entity`: `get_entity(entity)` → `EntityRef::archetype()` → `archetype.components()` → `Components::get_info(ComponentId)`. Daraus werden Name, `ComponentId`, optionale `TypeId` und Layout-Größe gewonnen und nach Name sortiert. Eine Komponente ohne `TypeId` wird angezeigt, aber als nicht reflektierbar gemeldet; eine Zero-Sized-Komponente erhält keine Wert-UI.

   Der konkrete reflektierte Zugriff lautet:

   ```text
   component TypeId
     -> RestrictedWorldView::split_off_component((entity, type_id))
     -> Components::get_id(type_id)
     -> EntityRef::get_mut_by_id(ComponentId)
     -> MutUntyped -> ReflectFromPtr::as_reflect_mut
     -> InspectorUi::for_bevy(...).ui_for_reflect_with_options(...)
   ```

   `get_entity_component_reflect` liefert dabei `ReflectBorrow::Mutable(Mut<dyn Reflect>)`. Ist der ECS-Komponent immutable, fällt der Code auf `get_by_id` und `ReflectFromPtr::as_reflect` mit `ReflectBorrow::Immutable(&dyn Reflect)` zurück. `mut_untyped_to_reflect` und `ptr_untyped_to_reflect` prüfen zuerst Registry-Eintrag und `ReflectFromPtr`; andernfalls entstehen typisierte Fehler (`NoTypeRegistration`, `NoTypeData`, fehlende Komponente).

   Für einen mutierbaren Wert ruft `ui_for_entity_components` `bypass_change_detection().as_partial_reflect_mut()` auf. `InspectorUi::ui_for_reflect_with_options` traversiert danach `Struct`, `TupleStruct`, `Tuple`, `List`, `Array`, `Map`, `Set` oder `Enum`; ein primitives beziehungsweise opakes Feld kann über einen registrierten `InspectorEguiImpl` behandelt werden. Liefert die UI `changed == true`, folgt ausdrücklich `value.set_changed()`. Die Mutation des Feldes ist zu diesem Zeitpunkt bereits direkt im ECS-Wert erfolgt; `set_changed` aktualisiert nur die Bevy-Change-Detection.

   Der Mehrfach-Entity-Pfad (`ui_for_entities_shared_components`) nimmt gemeinsame ComponentIds, verwendet `resources_components`, holt für jedes Entity reflektierte Mutable-Werte über `get_entity_component_reflect_unchecked`, lässt `ui_for_reflect_many_with_options` editieren und ruft bei Änderung für jeden Wert `set_changed()` auf. **Severity: hoch – fehlende `ReflectFromPtr`-/Registry-Daten verhindert den generischen Wertzugriff; Filterung und Metadatenentdeckung sind davon getrennt.**

4. **[Info] Resourcefluss: Registry-Entdeckung, Lesen und Schreiben.** — `ui_for_resources` iteriert nicht über nur vorhandene Resources, sondern über die `AppTypeRegistry`-Einträge und behält Einträge mit `ReflectResource`. Name (`short_path`) und `TypeId` werden sortiert; `by_type_id::ui_for_resource` prüft erst beim Zugriff, ob die Resource tatsächlich existiert.

   Der dynamische Resource-Pfad in [`bevy_inspector/mod.rs`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/bevy_inspector/mod.rs) lautet:

   ```text
   ReflectResource-Registration
     -> RestrictedWorldView::split_off_resource(type_id)
     -> Components::get_id(type_id)
     -> World::get_resource_mut_by_id(ComponentId)
     -> MutUntyped -> ReflectFromPtr::as_reflect_mut
     -> InspectorUi::ui_for_reflect(...)
     -> bei changed: resource.set_changed()
   ```

   `get_resource_reflect_mut_by_id` ist ein mutabler Pfad; er liefert keinen allgemeinen immutable Fallback. Der typed Pfad `ui_for_resource::<R>` nutzt `split_off_resource_typed::<R>`, eine echte `Mut<R>`, und dieselbe `bypass_change_detection`/`set_changed`-Sequenz. `ReflectResource` in Bevy [`crates/bevy_ecs/src/reflect/resource.rs`](https://github.com/bevyengine/bevy/blob/v0.19.0/crates/bevy_ecs/src/reflect/resource.rs) ist nur Marker-Datentyp; `ReflectFromPtr` und die Resource im World bleiben für den Wertzugriff erforderlich. Die Resource muss zudem vorher eingefügt/initialisiert sein; der Quick-Plugin-Code initialisiert sie nicht.

   `ui_for_state` ist eine Sonderform: Es splittet `State<T>` und `NextState<T>`, klont den aktuellen Zustand, editiert den Klon und schreibt bei Änderung `NextState::Pending(current)`. Das ist eine State-Transition und kein direktes Überschreiben des `State<T>`-Werts.

   **Severity: mittel – Registry-Iteration kann nicht vorhandene Resources auflisten, ohne einen `present`-Status zu führen.** Nicht-sendbare Resources liegen außerhalb des normalen `iter_resources`-Pfads und brauchen einen eigenen main-thread-konformen Zugriff; der geprüfte Inspector-All-Resources-Pfad behandelt sie nicht als separat ausweisbare Kategorie.

5. **[Info] Assetfluss: Asset-Typen, IDs, Lesen und Schreiben.** — `ui_for_all_assets` filtert die `TypeRegistry`-Einträge nach `ReflectAsset`, sortiert nach Short Path und ruft `by_type_id::ui_for_assets` auf. Dort werden `ReflectAsset` und die zugehörige `ReflectHandle`-TypeData verlangt. Danach liefert `reflect_asset.ids(world)` die live IDs des typisierten `Assets<T>`-Storages. `handle_name` bevorzugt `AssetServer::get_path`; ansonsten formatiert es Index- oder UUID-ID.

   Der genaue dynamische Ablauf für jeden `UntypedAssetId` ist:

   - **UUID-ID:** Mit `ReflectHandle::typed(UntypedHandle::Uuid { uuid, type_id })` wird ein typisierter, reflektierter Handle erzeugt. Eine `RestrictedWorldView::new(world)` wird an `InspectorUi` gegeben. Der `short_circuit` in [`bevy_inspector/mod.rs`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/bevy_inspector/mod.rs) erkennt den Handle, splittet `Assets<T>` aus der View, ruft `ReflectAsset::get_unchecked_mut` auf und editiert den dahinterliegenden Assetwert. Nach dem UUID-UI-Aufruf wird `queue.apply(world)` ausgeführt.
   - **Allgemeine/Index-ID:** Der Code kann einen `Handle<T>` nicht allgemein aus `ReflectAsset`/`ReflectHandle` erzeugen. Stattdessen splittet er `Assets<T>` mit `reflect_asset.assets_resource_type_id()` aus, ruft genau einmal `unsafe ReflectAsset::get_unchecked_mut(asset_world.world(), handle_id)` auf und editiert den resultierenden `&mut dyn Reflect` mit einer Rest-World-View. Das ist direkte In-Place-Mutation des Assets.
   - **Read-only Handle-Navigation:** `short_circuit_readonly` verwendet `ReflectAsset::get` auf einer zulässigen `Assets<T>`-View und traversiert den zurückgegebenen Wert immutable.
   - `ui_for_asset` prüft Registry, `ReflectAsset`, `ReflectHandle` und `ids`; die eigentliche Handle-UI-Mutation ist im gezeigten generischen Pfad nur für UUID-IDs implementiert, für Index-IDs liefert sie `false`.

   `ReflectAsset` und die Storage-APIs kommen aus Bevy [`crates/bevy_asset/src/reflect.rs`](https://github.com/bevyengine/bevy/blob/v0.19.0/crates/bevy_asset/src/reflect.rs) und [`crates/bevy_asset/src/assets.rs`](https://github.com/bevyengine/bevy/blob/v0.19.0/crates/bevy_asset/src/assets.rs). `AssetApp::register_asset_reflect::<A>()` muss aufgerufen werden; außerdem muss ein live `Assets<A>`-Storage existieren (typischerweise über `init_asset`/Asset-Plugin). `ReflectAsset::get_unchecked_mut` ist bewusst `unsafe` und nur mit der zuvor ausgesonderten `Assets<T>`-Ressource gültig.

   **Severity: hoch – `AssetId::Index` ist kein dauerhaftes Wire-Identifikationsmerkmal; UUIDs und Pfade haben getrennte Lebensdauer-/Verfügbarkeitssemantik.** **Zusätzlicher Quellbefund, Severity mittel:** Im direkten Index-/Assetwert-Zweig von `by_type_id::ui_for_assets` ist im geprüften Commit kein abschließendes `queue.apply(world)` wie im UUID-Zweig sichtbar. Direkte reflektierte Assetfeld-Mutationen finden trotzdem statt; ein JSON-Backend sollte deferred Commands in diesem Zweig nicht als ausgeführt voraussetzen und seine Queue-Lebensdauer explizit kontrollieren.

6. **[Info] Reflection- und Change-Detection-Details.** — `InspectorUi` in [`crates/bevy-inspector-egui/src/reflect_inspector/mod.rs`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/reflect_inspector/mod.rs) erhält eine `TypeRegistry`-Referenz und einen `Context` mit optionaler `RestrictedWorldView` und optionaler `&mut CommandQueue`. `InspectorUi::for_bevy` aktiviert die Bevy-spezifischen Short-Circuits für Handles/Assets; `new_no_short_circuit` kann keine Welt-Referenzen wie `Handle<StandardMaterial>` auflösen.

   Die UI arbeitet mit `PartialReflect`, `ReflectMut` und `ReflectRef`, nicht mit JSON. `try_as_reflect_mut` und registrierte `InspectorEguiImpl` bestimmen zunächst, ob ein konkreter Spezialrenderer existiert; andernfalls erfolgt die reflektierte Struktur-/Container-Traversierung. Opaque Werte ohne passende Inspector-Implementierung werden als nicht unterstützt angezeigt.

   `MutUntyped::map_unchanged` erhält die Verbindung zur ECS-Change-Detection. Der Inspector umgeht beim Traversieren absichtlich die automatische Markierung (`bypass_change_detection`) und ruft `set_changed` nur nach einer tatsächlichen UI-Änderung. `Mut::changed_by` wird zusätzlich für die Anzeige der letzten Änderungsstelle verwendet. Für Assets liefert `ReflectAsset::get_unchecked_mut` einen direkten mutablen Assetwert; der gezeigte Code ruft für diesen inneren Wert kein `set_changed` auf. Asset-Store-/Asset-Event-Semantik darf daher nicht mit normaler Component-Change-Detection gleichgesetzt werden.

   `CommandQueue` wird an verschachtelte Inspector-Operationen weitergereicht, während die aktuelle Resource/Component/`Assets<T>`-Exklusivität aufgespalten ist. Die Top-Level-Funktionen wenden die Queue typischerweise erst nach dem UI-Scope mit `queue.apply(world)` an. **Design-Implikation:** Ein eigener Befehl sollte direkte Reflected-Mutationen und deferred ECS-Kommandos getrennt bilanzieren, die Queue nach jedem abgeschlossenen Restricted-Borrow anwenden und Fehler beim Anwenden nicht stillschweigend in einen Erfolg verwandeln.

7. **[Info] Registrierung ist mehrstufig.** — Für einen dynamischen ECS-Wert sind mindestens die World-Registrierung und Reflection-TypeData erforderlich:

   - Komponente: `#[derive(Component, Reflect)]`, `#[reflect(Component)]` und `app.register_type::<C>()`; die Komponente muss im Entity vorhanden sein.
   - Resource: `#[derive(Resource, Reflect)]`, `#[reflect(Resource)]`, `app.register_type::<R>()` und eine eingefügte/initialisierte Resource. Für den geprüften generischen Mutationspfad müssen mutabler Resource-Zugriff und `ReflectFromPtr` vorhanden sein.
   - Asset: `A: Asset + Reflect`, `app.register_asset_reflect::<A>()`, `Assets<A>`-Storage und für UI-/Handle-Auflösung gegebenenfalls `AssetServer`/Pfadregistrierung.
   - Verschachtelte Felder: deren reflected TypeInfo/TypeData muss ebenfalls registriert sein; Handle-Felder benötigen `ReflectHandle` und `ReflectAsset` für den Short-Circuit.
   - UI-only: `DefaultInspectorConfigPlugin` liefert Standard-Inspector-Implementierungen; `EguiPlugin`, `MainSchedulePlugin` und `PrimaryEguiContext` werden nur vom Quick/UI-Pfad benötigt.

   **Severity: hoch – `#[derive(Reflect)]` allein ist kein Nachweis, dass ein Wert über `AppTypeRegistry` und `ReflectFromPtr` erreichbar oder mutierbar ist.**

8. **[Medium] Arbiträres JSON kann nicht automatisch in einen reflektierten Wert gemappt werden.** — Der geprüfte Inspector deserialisiert überhaupt kein JSON. Er erhält bereits einen typkorrekten `PartialReflect`-Baum und mutiert dessen Felder. Für eine externe JSON-Anbindung gibt es in Bevy `ReflectSerializer`/`TypedReflectSerializer` und `ReflectDeserializer` in [`crates/bevy_reflect/src/serde`](https://github.com/bevyengine/bevy/tree/v0.19.0/crates/bevy_reflect/src/serde); diese benötigen Registry-/Serde-TypeData und die Zieltypinformation. `ReflectFromPtr` liest nur aus einem bereits existierenden Speicherwert und konstruiert keinen Wert aus JSON.

   Grenzen sind insbesondere:

   - JSON enthält ohne separates Type Path/Schema keine Information, ob eine Zahl `u8`, `f32`, `Entity`, Enum-Discriminant oder ein benutzerdefinierter Opaque-Typ sein soll.
   - `ReflectDeserializer` kann nur registrierte und mit passender `ReflectDeserialize`-/Registry-TypeData versehene Typen erzeugen; `FromReflect`/`ReflectFromReflect` ist kein allgemeiner Beweis, dass jede JSON-Struktur akzeptiert wird.
   - Struct-Felder, Tuple-/Tuple-Struct-Indizes, Enum-Varianten, Map-Schlüssel, Sets und Collections haben unterschiedliche Form- und Typregeln. Fehlende Felder, unbekannte Felder und inkompatible Nummern benötigen eine explizite Policy.
   - Handles, `Entity`, `TypeId`, `ComponentId`, Pointer, `World`, GPU-Werte und andere Runtime-Identitäten sind keine stabilen JSON-Werte. Asset-Handles benötigen eine projektspezifische beziehungsweise Bevy-Handle-Serialisierungsstrategie; Index-IDs sind nicht dauerhaft.
   - `NaN`/Infinity, nicht-stringartige JSON-Map-Schlüssel und opake Bytes/Renderwerte sind in strengem JSON nicht verlustfrei darstellbar.

   **Design-Implikation, keine Produktentscheidung:** Ein JSON-Befehl sollte die Zielart separat und kanonisch benennen, vor der Mutation Registrierung/Existenz/Mutierbarkeit validieren, die Deserialisierung in einen temporären reflected Wert durchführen und erst nach erfolgreicher Typprüfung per `Reflect::apply`/passender `FromReflect`-Route anwenden. Der Fehlervertrag sollte Teilwerte und nicht angewandte Änderungen unterscheiden; die konkrete Feld-/Patch-Syntax bleibt eine offene Produktentscheidung.

## Ablauf als Referenz für ein JSON-Backend

Der bestätigte Inspector-Ablauf lässt sich ohne UI sinngemäß so modellieren:

```text
World + AppTypeRegistry
  -> kanonischen Type Path/Asset-Typ auflösen
  -> ComponentId bzw. Resource-/Assets-Storage und Existenz prüfen
  -> RestrictedWorldView für den mutierten Wert abtrennen
  -> ReflectFromPtr / ReflectAsset lesen oder mutabel erhalten
  -> temporären, typgeprüften PartialReflect-Wert validieren
  -> direkte Reflect-Mutation durchführen
  -> bei ECS-Mut: set_changed()
  -> deferred CommandQueue nach Borrow-Scope anwenden
  -> deterministischen Snapshot/Fehlerbericht erzeugen
```

**Design-Implikation:** Diese Abfolge beschreibt technische Schutz- und Validierungspunkte, legt aber weder konkrete JSON-Feldnamen noch Patch-/Query-Syntax fest. Entity-IDs, Type Paths und Asset-IDs sollten als externe Identitäten behandelt und nicht mit `TypeId`/`ComponentId` aus dem laufenden Prozess verwechselt werden.

## Quellen (nur primär/offiziell)

### Gepinnter `bevy-inspector-egui`-Stand

- [Commit `58782ac050dddd64527739215b50169ef2da83a4`](https://github.com/jakobhellermann/bevy-inspector-egui/commit/58782ac050dddd64527739215b50169ef2da83a4) — geprüfter Bevy-0.19-Kompatibilitätsstand.
- [`src/restricted_world_view.rs`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/restricted_world_view.rs) — Allow-/Forbid-Views, `UnsafeWorldCell`, `ReflectFromPtr`, Mutable-/Immutable-Fallback.
- [`src/bevy_inspector/mod.rs`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/bevy_inspector/mod.rs) — Entity-/Resource-/Asset-Entdeckung, reflektierte Mutation, Short-Circuits, `CommandQueue`.
- [`src/reflect_inspector/mod.rs`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/reflect_inspector/mod.rs) — `Context`, `InspectorUi`, `PartialReflect`-Traversal und `changed`-Vertrag.
- [`src/quick.rs`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/quick.rs) und [`src/lib.rs`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/src/lib.rs) — Plugin-/Registrierungsanforderungen und Standard-TypeData.
- [Gepinntes `Cargo.toml`](https://github.com/jakobhellermann/bevy-inspector-egui/blob/58782ac050dddd64527739215b50169ef2da83a4/crates/bevy-inspector-egui/Cargo.toml) — Bevy-0.19-Abhängigkeiten.

### Bevy 0.19.0

- [`bevy_ecs/src/reflect/component.rs`](https://github.com/bevyengine/bevy/blob/v0.19.0/crates/bevy_ecs/src/reflect/component.rs) und [`resource.rs`](https://github.com/bevyengine/bevy/blob/v0.19.0/crates/bevy_ecs/src/reflect/resource.rs) — `ReflectComponent`, `ReflectResource` und ECS-Reflection.
- [`bevy_ecs/src/world/mod.rs`](https://github.com/bevyengine/bevy/blob/v0.19.0/crates/bevy_ecs/src/world/mod.rs) und [`world/unsafe_world_cell.rs`](https://github.com/bevyengine/bevy/blob/v0.19.0/crates/bevy_ecs/src/world/unsafe_world_cell.rs) — Entity-/Resource-Pointerzugriff, `ComponentId`, Resource- und Change-Detection-APIs.
- [`bevy_reflect/src/type_registry.rs`](https://github.com/bevyengine/bevy/blob/v0.19.0/crates/bevy_reflect/src/type_registry.rs) — `TypeRegistry`, `ReflectFromPtr` und Registry-TypeData.
- [`bevy_reflect/src/serde`](https://github.com/bevyengine/bevy/tree/v0.19.0/crates/bevy_reflect/src/serde) — `ReflectSerializer`, `TypedReflectSerializer`, `ReflectDeserializer`.
- [`bevy_asset/src/reflect.rs`](https://github.com/bevyengine/bevy/blob/v0.19.0/crates/bevy_asset/src/reflect.rs), [`assets.rs`](https://github.com/bevyengine/bevy/blob/v0.19.0/crates/bevy_asset/src/assets.rs) und [`id.rs`](https://github.com/bevyengine/bevy/blob/v0.19.0/crates/bevy_asset/src/id.rs) — `ReflectAsset`, Asset-Storage, IDs und Handles.

### Lokale Repository-Quelle

- [`crates/bug_hunter/docs/api/research/bevy-screenshot-without-simulation-tick.md`](./bevy-screenshot-without-simulation-tick.md) — Stilreferenz; diese Recherche ändert keine weitere Projektdatei.
- [`crates/bug_hunter/Cargo.toml`](../../../Cargo.toml) — lokale Bevy-0.19-Anforderung und aktueller Dependency-Stand.

## Lücken und Rest-Risiken

- **[Hoch]** Der geprüfte Inspector-Stand ist ein Commit-Pin; vor einer tatsächlichen Dependency-Aufnahme muss ein veröffentlichter Release mit Bevy 0.19 geprüft oder der Commit bewusst gepinnt werden.
- **[Hoch]** Es wurde kein lokaler Compile-/Runtime-Test des Commit-Stands ausgeführt. Die Befunde sind Quellcode- und API-Analyse, keine Integrationsfreigabe.
- **[Mittel]** Die genaue Asset-Event-/Change-Detection-Semantik einer direkten `ReflectAsset::get_unchecked_mut`-Mutation ist nicht dieselbe wie `Mut<T>::set_changed`; ein JSON-Write muss hierfür einen expliziten Vertrag wählen.
- **[Mittel]** Der direkte Index-Assetzweig des geprüften `by_type_id::ui_for_assets` wendet seine lokale `CommandQueue` nicht sichtbar mit `queue.apply(world)` an; deferred Commands dürfen dort nicht als garantiert ausgeführt gelten.
- **[Mittel]** Nicht-sendbare Resources, nicht registrierte dynamische Komponenten, fehlende nested TypeData, Handles ohne stabile Identität und opake Render-/GPU-Werte benötigen zusätzliche Fehler-/Auslassungsregeln.
- **[Niedrig]** Die UI registriert zusätzliche Standard-Inspector-Implementierungen; ein eigener JSON-Backend kann diese UI-Registrierung weglassen, braucht aber eigene Typ-/Serde-Policies.

**Vertrauen:** hoch für den dokumentierten Mutation-/Borrow-Fluss in Commit `58782ac`; mittel für daraus abgeleitete Asset-Event- und JSON-Produktfolgen, die nicht durch den Inspector selbst festgelegt werden.

## Akzeptanzbericht

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Konkrete bestätigte Mutationflows mit Schweregraden und Pfaden: restricted_world_view.rs (Borrow-/Pointer-Schutz), bevy_inspector/mod.rs (Entity-, Resource- und Asset-Zugriff), reflect_inspector/mod.rs (PartialReflect/changed/CommandQueue), quick.rs/lib.rs (Registrierung/UI) sowie Bevy-0.19-Quellen für Reflection, World und Assets."
    }
  ],
  "changedFiles": [
    "crates/bug_hunter/docs/api/research/bevy-world-inspection-and-mutation.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [],
  "validationOutput": [
    "Stilreferenz unter crates/bug_hunter/docs/api/research/bevy-screenshot-without-simulation-tick.md gelesen.",
    "Der deutsche Forschungsbrief wurde am vorgeschriebenen absoluten Zielpfad geschrieben; keine andere Projektdatei wurde editiert."
  ],
  "residualRisks": [
    "hoch: Commit-Pin statt verifiziertem veröffentlichtem Bevy-0.19-Release",
    "hoch: kein lokaler Compile-/Runtime-Test des Inspector-Commits",
    "mittel: direkte Asset-Mutation und CommandQueue-/Asset-Event-Semantik",
    "mittel: fehlende Registry-/Serde-TypeData, non-send Resources und opake Werte"
  ],
  "noStagedFiles": true,
  "diffSummary": "Nur der vorgeschriebene deutsche Forschungsbrief wurde neu geschrieben; keine andere Projektdatei geändert.",
  "reviewFindings": [
    "blocker: none",
    "high: crates/bug_hunter/docs/api/research/bevy-world-inspection-and-mutation.md: Registrierung und fehlende ReflectFromPtr-/ReflectAsset-TypeData blockieren generische Mutation.",
    "medium: crates/bug_hunter/docs/api/research/bevy-world-inspection-and-mutation.md: direkter Index-Assetzweig zeigt kein abschließendes queue.apply(world); deferred Commands dort nicht voraussetzen.",
    "medium: crates/bug_hunter/docs/api/research/bevy-world-inspection-and-mutation.md: Arbiträres JSON ist ohne Typ-/Serde-Schema nicht sicher in PartialReflect überführbar."
  ],
  "manualNotes": "Die Dokumentation trennt bestätigte Quellbefunde von Design-Implikationen und entscheidet keine konkrete Produkt- oder JSON-Schnittstelle."
}
```