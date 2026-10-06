import AppKit

final class KeyDeckWindow: NSWindow {
    override var canBecomeKey: Bool { true }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)

let screens = NSScreen.screens.map(\.frame)
guard var desktop = screens.first else {
    fputs("no NSScreen\n", stderr)
    exit(2)
}
for frame in screens.dropFirst() {
    desktop = desktop.union(frame)
}

let tiny = NSRect(x: desktop.maxX - 8, y: desktop.maxY - 8, width: 4, height: 4)
let deck = KeyDeckWindow(
    contentRect: tiny,
    styleMask: [.borderless],
    backing: .buffered,
    defer: false
)
deck.backgroundColor = .black
deck.isOpaque = true
deck.ignoresMouseEvents = true
deck.level = .floating

func acknowledge(_ state: String) {
    print("{\"deck_state\":\"\(state)\"}")
    fflush(stdout)
}

DispatchQueue.global(qos: .userInitiated).async {
    while let command = readLine() {
        DispatchQueue.main.sync {
            switch command {
            case "small":
                deck.setFrame(tiny, display: true)
                deck.makeKeyAndOrderFront(nil)
                app.activate(ignoringOtherApps: true)
                acknowledge("small")
            case "cover":
                deck.setFrame(desktop, display: true)
                deck.makeKeyAndOrderFront(nil)
                app.activate(ignoringOtherApps: true)
                acknowledge("cover")
            case "hidden":
                deck.orderOut(nil)
                acknowledge("hidden")
            case "quit":
                deck.orderOut(nil)
                acknowledge("quit")
                app.terminate(nil)
            default:
                acknowledge("unknown")
            }
        }
    }
}

app.run()
