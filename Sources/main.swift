import AppKit
import ImageIO

// MARK: - Sprite data (PMD SpriteCollab format, parsed from AnimData.xml)

struct PixelBox {
    var minX: Int, minY: Int, maxX: Int, maxY: Int   // y-down, max exclusive, relative to frame center
    var width: Int { maxX - minX }
    var height: Int { maxY - minY }
    func union(_ o: PixelBox) -> PixelBox {
        PixelBox(minX: min(minX, o.minX), minY: min(minY, o.minY), maxX: max(maxX, o.maxX), maxY: max(maxY, o.maxY))
    }
}

struct Animation {
    let name: String
    let source: String          // sheet this resolves to after following CopyOf
    let frameWidth: Int
    let frameHeight: Int
    let durations: [Int]        // ticks, 60 per second
    let rows: Int
    let frames: [[CGImage]]     // [row][column]

    var totalTicks: Int { durations.reduce(0, +) }

    /// Direction rows: 0 Down, 1 DownRight, 2 Right, 3 UpRight, 4 Up, 5 UpLeft, 6 Left, 7 DownLeft.
    func frame(row: Int, column: Int) -> CGImage { frames[rows == 1 ? 0 : row % rows][column] }

    /// Column at an elapsed tick count, or nil once a non-looping animation has finished.
    func column(atTick tick: Int, loop: Bool) -> Int? {
        if !loop && tick >= totalTicks { return nil }
        var t = loop ? tick % totalTicks : tick
        for (i, d) in durations.enumerated() {
            if t < d { return i }
            t -= d
        }
        return durations.count - 1
    }
}

struct SpriteError: Error, CustomStringConvertible {
    let description: String
}

private extension XMLElement {
    func text(_ name: String) -> String? {
        elements(forName: name).first?.stringValue?.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

/// RGBA pixels of an image, top row first.
func rgbaPixels(_ img: CGImage) -> [UInt8] {
    var data = [UInt8](repeating: 0, count: img.width * img.height * 4)
    let ctx = CGContext(data: &data, width: img.width, height: img.height, bitsPerComponent: 8,
                        bytesPerRow: img.width * 4, space: CGColorSpaceCreateDeviceRGB(),
                        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    ctx.draw(img, in: CGRect(x: 0, y: 0, width: img.width, height: img.height))
    return data
}

/// Bounds of the non-transparent pixels in a frame, relative to the frame center.
func opaqueBounds(_ img: CGImage) -> PixelBox? {
    let px = rgbaPixels(img)
    var box: PixelBox?
    for y in 0..<img.height {
        for x in 0..<img.width where px[(y * img.width + x) * 4 + 3] > 0 {
            let p = PixelBox(minX: x, minY: y, maxX: x + 1, maxY: y + 1)
            box = box?.union(p) ?? p
        }
    }
    guard var b = box else { return nil }
    let cx = img.width / 2, cy = img.height / 2
    b.minX -= cx; b.maxX -= cx; b.minY -= cy; b.maxY -= cy
    return b
}

final class SpriteSet {
    let order: [String]
    let animations: [String: Animation]

    subscript(name: String) -> Animation? { animations[name] }

    init(directory: URL) throws {
        let xmlURL = directory.appendingPathComponent("AnimData.xml")
        let doc = try XMLDocument(contentsOf: xmlURL)
        guard let nodes = try doc.nodes(forXPath: "/AnimData/Anims/Anim") as? [XMLElement], !nodes.isEmpty else {
            throw SpriteError(description: "No <Anim> entries in \(xmlURL.path)")
        }

        var raw: [String: XMLElement] = [:]
        var order: [String] = []
        for node in nodes {
            guard let name = node.text("Name") else { throw SpriteError(description: "<Anim> without <Name>") }
            raw[name] = node
            order.append(name)
        }

        var sheets: [String: CGImage] = [:]
        var animations: [String: Animation] = [:]
        for name in order {
            var source = name
            var seen: Set<String> = [name]
            while let target = raw[source]?.text("CopyOf") {
                guard raw[target] != nil, seen.insert(target).inserted else {
                    throw SpriteError(description: "\(name): bad CopyOf chain at \(target)")
                }
                source = target
            }
            let el = raw[source]!
            guard let fw = el.text("FrameWidth").flatMap(Int.init),
                  let fh = el.text("FrameHeight").flatMap(Int.init), fw > 0, fh > 0 else {
                throw SpriteError(description: "\(source): missing FrameWidth/FrameHeight")
            }
            let durations = (el.elements(forName: "Durations").first?.elements(forName: "Duration") ?? [])
                .compactMap { $0.stringValue.flatMap { Int($0.trimmingCharacters(in: .whitespaces)) } }
            guard !durations.isEmpty else { throw SpriteError(description: "\(source): no Durations") }

            if sheets[source] == nil {
                let url = directory.appendingPathComponent("\(source)-Anim.png")
                guard let src = CGImageSourceCreateWithURL(url as CFURL, nil),
                      let img = CGImageSourceCreateImageAtIndex(src, 0, nil) else {
                    throw SpriteError(description: "Missing sheet \(url.lastPathComponent)")
                }
                sheets[source] = img
            }
            let sheet = sheets[source]!
            guard sheet.width == fw * durations.count, sheet.height % fh == 0 else {
                throw SpriteError(description: "\(source): sheet is \(sheet.width)x\(sheet.height), expected "
                                  + "\(durations.count) columns of \(fw)x\(fh)")
            }
            let rows = sheet.height / fh
            let frames = (0..<rows).map { r in
                (0..<durations.count).map { c in
                    sheet.cropping(to: CGRect(x: c * fw, y: r * fh, width: fw, height: fh))!
                }
            }
            animations[name] = Animation(name: name, source: source, frameWidth: fw, frameHeight: fh,
                                         durations: durations, rows: rows, frames: frames)
        }
        self.order = order
        self.animations = animations
    }

    /// Union of opaque pixels over every frame and direction of the given animations.
    func contentBounds(of names: [String]) -> PixelBox {
        var box: PixelBox?
        for name in names {
            guard let anim = animations[name] else { continue }
            for row in anim.frames {
                for frame in row {
                    if let b = opaqueBounds(frame) { box = box?.union(b) ?? b }
                }
            }
        }
        return box ?? PixelBox(minX: -1, minY: -1, maxX: 1, maxY: 1)
    }
}

/// Draws a frame with its center at `anchor` (sprite pixels, y-up) at an integer scale. `factor` is how much
/// the image was already upscaled; the rest of the way (scale / factor) is nearest-neighbor.
func drawFrame(_ img: CGImage, factor: Int = 1, in ctx: CGContext, anchorX: Int, anchorY: Int, scale: Int) {
    ctx.saveGState()
    ctx.interpolationQuality = .none
    ctx.setShouldAntialias(false)
    let w = img.width / factor, h = img.height / factor
    let x = (anchorX - w / 2) * scale
    let y = (anchorY - (h - h / 2)) * scale
    ctx.draw(img, in: CGRect(x: x, y: y, width: w * scale, height: h * scale))
    ctx.restoreGState()
}

// MARK: - Upscaling

enum Style: String, CaseIterable {
    case pixel = "Pixel (nearest)"
    case scaleNx = "Scale2x/3x"
    case mmpx = "MMPX"
    case xbrz = "xBRZ"
}

private let argbBitmapInfo = CGBitmapInfo.byteOrder32Little.rawValue | CGImageAlphaInfo.premultipliedFirst.rawValue

/// 0xAARRGGBB pixels, top row first. Sprite alpha is 0 or 255, so premultiplied and straight are the same here.
func argbPixels(_ img: CGImage) -> [UInt32] {
    var px = [UInt32](repeating: 0, count: img.width * img.height)
    let ctx = CGContext(data: &px, width: img.width, height: img.height, bitsPerComponent: 8,
                        bytesPerRow: img.width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: argbBitmapInfo)!
    ctx.draw(img, in: CGRect(x: 0, y: 0, width: img.width, height: img.height))
    return px
}

/// Builds an image from straight-alpha 0xAARRGGBB pixels (scalers can produce partial alpha at edges).
func imageFromARGB(_ px: [UInt32], width: Int, height: Int) -> CGImage {
    var pre = px.map { c -> UInt32 in
        let a = c >> 24
        if a == 255 { return c }
        let r = ((c >> 16) & 0xFF) * a / 255, g = ((c >> 8) & 0xFF) * a / 255, b = (c & 0xFF) * a / 255
        return (a << 24) | (r << 16) | (g << 8) | b
    }
    let ctx = CGContext(data: &pre, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
                        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: argbBitmapInfo)!
    return ctx.makeImage()!
}

/// Scale2x / Scale3x (EPX family, Andrea Mazzoleni's rules). Neighbors past the edge clamp to the edge.
func scaleNx(_ src: [UInt32], width w: Int, height h: Int, factor n: Int) -> [UInt32] {
    var dst = [UInt32](repeating: 0, count: w * n * h * n)
    func at(_ x: Int, _ y: Int) -> UInt32 { src[min(max(y, 0), h - 1) * w + min(max(x, 0), w - 1)] }
    for y in 0..<h {
        for x in 0..<w {
            let A = at(x - 1, y - 1), B = at(x, y - 1), C = at(x + 1, y - 1)
            let D = at(x - 1, y), E = at(x, y), F = at(x + 1, y)
            let G = at(x - 1, y + 1), H = at(x, y + 1), I = at(x + 1, y + 1)
            var out = [UInt32](repeating: E, count: n * n)
            if B != H && D != F {
                if n == 2 {
                    out = [D == B ? D : E, B == F ? F : E,
                           D == H ? D : E, H == F ? F : E]
                } else {
                    out = [D == B ? D : E,
                           (D == B && E != C) || (B == F && E != A) ? B : E,
                           B == F ? F : E,
                           (D == B && E != G) || (D == H && E != A) ? D : E,
                           E,
                           (B == F && E != I) || (H == F && E != C) ? F : E,
                           D == H ? D : E,
                           (D == H && E != I) || (H == F && E != G) ? H : E,
                           H == F ? F : E]
                }
            }
            for j in 0..<n { for i in 0..<n { dst[(y * n + j) * w * n + x * n + i] = out[j * n + i] } }
        }
    }
    return dst
}

/// Upscales one frame with `style`, using the largest factor the algorithm supports that divides `scale`.
/// Returns the image and the factor applied; the caller draws the remainder with nearest-neighbor.
func upscale(_ img: CGImage, style: Style, scale: Int) -> (image: CGImage, factor: Int) {
    if style == .pixel { return (img, 1) }
    var px = argbPixels(img), w = img.width, h = img.height, factor = 1
    func apply(_ n: Int, _ f: ([UInt32], Int, Int) -> [UInt32]) {
        px = f(px, w, h); w *= n; h *= n; factor *= n
    }
    switch style {
    case .pixel:
        break
    case .scaleNx:
        while (scale / factor) % 2 == 0 { apply(2) { scaleNx($0, width: $1, height: $2, factor: 2) } }
        while (scale / factor) % 3 == 0 { apply(3) { scaleNx($0, width: $1, height: $2, factor: 3) } }
    case .mmpx:
        while (scale / factor) % 2 == 0 {
            apply(2) { src, sw, sh in
                var dst = [UInt32](repeating: 0, count: sw * sh * 4)
                mmpx_scale2x(src, &dst, UInt32(sw), UInt32(sh))
                return dst
            }
        }
    case .xbrz:
        if let n = (2...6).reversed().first(where: { scale % $0 == 0 }) {
            apply(n) { src, sw, sh in
                var dst = [UInt32](repeating: 0, count: sw * sh * n * n)
                wob_xbrz_scale(Int32(n), src, &dst, Int32(sw), Int32(sh))
                return dst
            }
        }
    }
    return (factor == 1 ? img : imageFromARGB(px, width: w, height: h), factor)
}

// MARK: - Pet behavior

enum PetMode: CaseIterable {
    case idle, salute, landing, dragging
}

/// Which animation each pet state plays. Names must exist in AnimData.xml; missing ones fall back to Idle.
let stateAnimations: [PetMode: (name: String, loop: Bool)] = [
    .idle: ("Idle", true),
    .salute: ("Withdraw", false),   // brace, then arm up + mouth open: the "Wobbuffet!" pose
    .landing: ("Hop", false),       // bounce after being dropped
    .dragging: ("Walk", true),      // faces the direction he's being carried
]

final class PetState {
    private(set) var mode: PetMode = .idle
    private(set) var modeStart = Date()
    var facing = 0

    func set(_ m: PetMode) {
        mode = m
        modeStart = Date()
    }

    func salute() {
        if mode == .idle || mode == .landing { set(.salute) }
    }

    /// Screen-space drag delta (y up) to a direction row.
    func face(dx: CGFloat, dy: CGFloat) {
        let angle = atan2(-dy, dx) * 180 / .pi          // 0 right, 90 down, 180 left, -90 up
        facing = ((2 - Int((angle / 45).rounded())) % 8 + 8) % 8
    }
}

// MARK: - Window

final class PetView: NSView {
    var onClick: (() -> Void)?
    var onDragBegan: (() -> Void)?
    var onDragMoved: ((CGFloat, CGFloat) -> Void)?
    var onDrop: (() -> Void)?
    var frameImage: CGImage? { didSet { if frameImage !== oldValue { needsDisplay = true } } }
    var frameFactor = 1
    var anchor = (x: 0, y: 0)
    var spriteScale = 3 { didSet { needsDisplay = true } }

    private var dragStartMouse = NSPoint.zero
    private var dragStartOrigin = NSPoint.zero
    private var lastSample = NSPoint.zero
    private var dragged = false

    override func hitTest(_ point: NSPoint) -> NSView? { frame.contains(point) ? self : nil }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        guard let ctx = NSGraphicsContext.current?.cgContext else { return }
        ctx.clear(bounds)
        if let img = frameImage { drawFrame(img, factor: frameFactor, in: ctx, anchorX: anchor.x, anchorY: anchor.y, scale: spriteScale) }
    }

    override func mouseDown(with event: NSEvent) {
        dragStartMouse = NSEvent.mouseLocation
        lastSample = dragStartMouse
        dragStartOrigin = window?.frame.origin ?? .zero
        dragged = false
    }

    override func mouseDragged(with event: NSEvent) {
        let p = NSEvent.mouseLocation
        let dx = p.x - dragStartMouse.x, dy = p.y - dragStartMouse.y
        if !dragged && abs(dx) + abs(dy) > 3 {
            dragged = true
            onDragBegan?()
        }
        guard dragged else { return }
        window?.setFrameOrigin(NSPoint(x: (dragStartOrigin.x + dx).rounded(), y: (dragStartOrigin.y + dy).rounded()))
        let sx = p.x - lastSample.x, sy = p.y - lastSample.y
        if hypot(sx, sy) >= 6 {
            onDragMoved?(sx, sy)
            lastSample = p
        }
    }

    override func mouseUp(with event: NSEvent) {
        dragged ? onDrop?() : onClick?()
    }
}

func spritesDirectory() -> URL {
    Bundle.main.resourceURL!.appendingPathComponent("sprites")
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSPanel!
    private var pet: PetView!
    private var sprites: SpriteSet!
    private var mapping: [PetMode: (name: String, loop: Bool)] = [:]
    private var content = PixelBox(minX: 0, minY: 0, maxX: 0, maxY: 0)
    private let state = PetState()
    private var saluteTimer: Timer?
    private var frameTimer: Timer?
    private let scales = [3, 4, 6, 8]
    private var rendered: [String: (frames: [[CGImage]], factor: Int)] = [:]
    private var sizeMenu = NSMenu()
    private var styleMenu = NSMenu()

    private var spriteScale: Int {
        get { let v = UserDefaults.standard.integer(forKey: "scale"); return scales.contains(v) ? v : 4 }
        set { UserDefaults.standard.set(newValue, forKey: "scale") }
    }

    private var style: Style {
        get { UserDefaults.standard.string(forKey: "style").flatMap(Style.init(rawValue:)) ?? .xbrz }
        set { UserDefaults.standard.set(newValue.rawValue, forKey: "style") }
    }

    /// Upscales every frame the pet can show for the current style and size.
    private func renderFrames() {
        rendered = [:]
        for name in Set(mapping.values.map(\.name)) {
            let anim = sprites[name]!
            var factor = 1
            let frames = anim.frames.map { row in
                row.map { frame -> CGImage in
                    let out = upscale(frame, style: style, scale: spriteScale)
                    factor = out.factor
                    return out.image
                }
            }
            rendered[name] = (frames, factor)
        }
        for item in sizeMenu.items { item.state = item.tag == spriteScale ? .on : .off }
        for item in styleMenu.items { item.state = item.title == style.rawValue ? .on : .off }
    }

    private var canvasSize: CGSize {
        CGSize(width: content.width * spriteScale, height: content.height * spriteScale)
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        do {
            sprites = try SpriteSet(directory: spritesDirectory())
        } catch {
            let alert = NSAlert()
            alert.messageText = "Couldn't load Wobbuffet sprites"
            alert.informativeText = "\(error)"
            alert.runModal()
            NSApp.terminate(nil)
            return
        }
        for mode in PetMode.allCases {
            let wanted = stateAnimations[mode]!
            if sprites[wanted.name] == nil {
                FileHandle.standardError.write(Data("No \(wanted.name) animation for \(mode); using Idle\n".utf8))
                mapping[mode] = ("Idle", wanted.loop)
            } else {
                mapping[mode] = wanted
            }
        }
        content = sprites.contentBounds(of: Set(mapping.values.map(\.name)).sorted())

        let size = canvasSize
        let visible = (NSScreen.main ?? NSScreen.screens[0]).visibleFrame
        var origin = NSPoint(x: (visible.maxX - size.width - 24).rounded(), y: visible.minY.rounded())
        if let saved = UserDefaults.standard.string(forKey: "origin") {
            let p = NSPointFromString(saved)
            let center = NSPoint(x: p.x + size.width / 2, y: p.y + size.height / 2)
            if NSScreen.screens.contains(where: { $0.frame.contains(center) }) { origin = p }
        }

        window = NSPanel(contentRect: NSRect(origin: origin, size: size),
                         styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        window.isOpaque = false
        window.backgroundColor = .clear
        window.hasShadow = false
        window.level = .floating
        window.hidesOnDeactivate = false
        window.collectionBehavior = [.canJoinAllSpaces, .stationary, .fullScreenAuxiliary, .ignoresCycle]

        pet = PetView(frame: NSRect(origin: .zero, size: size))
        pet.autoresizingMask = [.width, .height]
        pet.anchor = (x: -content.minX, y: content.maxY)
        pet.spriteScale = spriteScale
        pet.onClick = { [weak self] in self?.state.salute() }
        pet.onDragBegan = { [weak self] in self?.state.set(.dragging) }
        pet.onDragMoved = { [weak self] dx, dy in self?.state.face(dx: dx, dy: dy) }
        pet.onDrop = { [weak self] in
            self?.state.set(.landing)
            self?.saveOrigin()
        }
        pet.menu = buildMenu()
        renderFrames()

        window.contentView = pet
        tick()
        window.orderFrontRegardless()

        let timer = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in self?.tick() }
        RunLoop.main.add(timer, forMode: .common)
        frameTimer = timer

        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak self] in self?.state.salute() }
        scheduleSalute()
    }

    private func tick() {
        let current = mapping[state.mode]!
        let anim = sprites[current.name]!
        let ticks = Int(Date().timeIntervalSince(state.modeStart) * 60)
        guard let column = anim.column(atTick: ticks, loop: current.loop) else {
            state.set(.idle)
            return tick()
        }
        let r = rendered[current.name]!
        pet.frameFactor = r.factor
        pet.frameImage = r.frames[anim.rows == 1 ? 0 : state.facing % anim.rows][column]
    }

    private func scheduleSalute() {
        saluteTimer = Timer.scheduledTimer(withTimeInterval: .random(in: 10...25), repeats: false) { [weak self] _ in
            self?.state.salute()
            self?.scheduleSalute()
        }
    }

    private func saveOrigin() {
        UserDefaults.standard.set(NSStringFromPoint(window.frame.origin), forKey: "origin")
    }

    private func buildMenu() -> NSMenu {
        let menu = NSMenu()
        menu.addItem(withTitle: "Salute!", action: #selector(saluteNow), keyEquivalent: "").target = self
        menu.addItem(.separator())
        let styleItem = NSMenuItem(title: "Style", action: nil, keyEquivalent: "")
        for s in Style.allCases {
            styleMenu.addItem(withTitle: s.rawValue, action: #selector(setStyle(_:)), keyEquivalent: "").target = self
        }
        styleItem.submenu = styleMenu
        menu.addItem(styleItem)
        let sizeItem = NSMenuItem(title: "Size", action: nil, keyEquivalent: "")
        for scale in scales {
            let item = NSMenuItem(title: "\(scale)×", action: #selector(setScale(_:)), keyEquivalent: "")
            item.tag = scale
            item.target = self
            sizeMenu.addItem(item)
        }
        sizeItem.submenu = sizeMenu
        menu.addItem(sizeItem)
        menu.addItem(.separator())
        menu.addItem(withTitle: "Credits…", action: #selector(showCredits), keyEquivalent: "").target = self
        menu.addItem(withTitle: "Quit Wobbuffet", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "")
        return menu
    }

    @objc private func saluteNow() { state.salute() }

    @objc private func showCredits() {
        if let url = Bundle.main.url(forResource: "CREDITS", withExtension: "md") { NSWorkspace.shared.open(url) }
    }

    @objc private func setStyle(_ sender: NSMenuItem) {
        guard let s = Style(rawValue: sender.title) else { return }
        style = s
        renderFrames()
        tick()
    }

    @objc private func setScale(_ sender: NSMenuItem) {
        spriteScale = sender.tag
        pet.spriteScale = sender.tag
        renderFrames()
        tick()
        let old = window.frame
        let size = canvasSize
        window.setFrame(NSRect(x: (old.midX - size.width / 2).rounded(), y: old.minY,
                               width: size.width, height: size.height), display: true)
        saveOrigin()
    }
}

// MARK: - Contact sheet (first frame of every animation, rendered with drawFrame)

func renderContactSheet(to path: String) throws {
    let sprites = try SpriteSet(directory: spritesDirectory())
    let scale = 4, pad = 12, label = 18, columns = 5
    let names = sprites.order
    let cellW = names.map { sprites[$0]!.frameWidth }.max()! * scale + pad * 2
    let cellH = names.map { sprites[$0]!.frameHeight }.max()! * scale + pad * 2 + label
    let rowsN = (names.count + columns - 1) / columns
    let W = cellW * columns, H = cellH * rowsN

    let ctx = CGContext(data: nil, width: W, height: H, bitsPerComponent: 8, bytesPerRow: W * 4,
                        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    ctx.setFillColor(CGColor(gray: 1, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: W, height: H))
    NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: false)

    var problems: [String] = []
    print("name      frame  frame0 opaque box (rel. center)   blocks")
    for (i, name) in names.enumerated() {
        let anim = sprites[name]!
        let img = anim.frame(row: 0, column: 0)
        let col = i % columns, row = i / columns
        let cellX = col * cellW, cellY = H - (row + 1) * cellH
        // Frame cell, outlined so clipping/offset is visible.
        let fx = (cellX + (cellW - anim.frameWidth * scale) / 2) / scale * scale   // keep cells on the pixel grid
        let fy = (cellY + label + pad + scale - 1) / scale * scale
        let frameRect = CGRect(x: fx, y: fy, width: anim.frameWidth * scale, height: anim.frameHeight * scale)
        ctx.setFillColor(CGColor(gray: 0.93, alpha: 1))
        ctx.fill(frameRect)
        ctx.setStrokeColor(CGColor(red: 0.85, green: 0.2, blue: 0.2, alpha: 1))
        ctx.setLineWidth(1)
        ctx.stroke(frameRect.insetBy(dx: -0.5, dy: -0.5))
        drawFrame(img, in: ctx, anchorX: fx / scale + img.width / 2, anchorY: fy / scale + (img.height - img.height / 2),
                  scale: scale)
        let text = "\(name)\(anim.source != name ? " → \(anim.source)" : "")  \(anim.frameWidth)×\(anim.frameHeight)"
        (text as NSString).draw(at: NSPoint(x: cellX + pad, y: cellY + 4),
                                withAttributes: [.font: NSFont.systemFont(ofSize: 12), .foregroundColor: NSColor.black])

        // Crispness check: every scale×scale block inside the frame must be one solid color.
        let data = ctx.data!.assumingMemoryBound(to: UInt32.self)
        var blurred = 0
        for py in 0..<(anim.frameHeight * scale) {
            for px in 0..<(anim.frameWidth * scale) {
                let x = fx + px, yUp = fy + py
                let bx = fx + px / scale * scale, byUp = fy + py / scale * scale
                if data[(H - 1 - yUp) * W + x] != data[(H - 1 - byUp) * W + bx] { blurred += 1 }
            }
        }
        let box = opaqueBounds(img)
        let boxText = box.map { "x \($0.minX)..<\($0.maxX)  y \($0.minY)..<\($0.maxY)" } ?? "empty"
        print("\(name.padding(toLength: 9, withPad: " ", startingAt: 0)) \(anim.frameWidth)x\(anim.frameHeight)  "
              + "\(boxText.padding(toLength: 32, withPad: " ", startingAt: 0)) \(blurred == 0 ? "crisp" : "\(blurred) off")")
        if blurred > 0 { problems.append("\(name): \(blurred) non-uniform pixels") }
        if let b = box, b.minX <= -anim.frameWidth / 2 || b.maxX >= anim.frameWidth / 2
            || b.minY <= -anim.frameHeight / 2 || b.maxY >= anim.frameHeight / 2 {
            problems.append("\(name): sprite reaches the frame edge in the source sheet (check visually)")
        }
    }
    let out = ctx.makeImage()!
    let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: path) as CFURL, "public.png" as CFString, 1, nil)!
    CGImageDestinationAddImage(dest, out, nil)
    guard CGImageDestinationFinalize(dest) else { throw SpriteError(description: "Couldn't write \(path)") }
    print(problems.isEmpty ? "No problems found." : problems.joined(separator: "\n"))
}

// MARK: - Style comparison (same frames through every upscaler, drawn with drawFrame)

func renderComparison(to path: String, scale: Int) throws {
    let sprites = try SpriteSet(directory: spritesDirectory())
    let picks: [(String, Int, Int)] = [("Idle", 0, 0), ("Withdraw", 0, 5), ("Walk", 2, 0)]   // (anim, row, column)
    let frames = try picks.map { name, row, col -> CGImage in
        guard let a = sprites[name] else { throw SpriteError(description: "No \(name) animation") }
        return a.frame(row: row, column: col)
    }
    let box = frames.compactMap(opaqueBounds).reduce(PixelBox(minX: 0, minY: 0, maxX: 0, maxY: 0)) { $0.union($1) }
    let pad = 16, header = 28, label = 120
    let cellW = (box.width + 4) * scale, cellH = (box.height + 4) * scale
    let W = label + Style.allCases.count * (cellW + pad) + pad
    let H = header + frames.count * (cellH + pad) + pad

    let ctx = CGContext(data: nil, width: W, height: H, bitsPerComponent: 8, bytesPerRow: W * 4,
                        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    ctx.setFillColor(CGColor(gray: 1, alpha: 1))
    ctx.fill(CGRect(x: 0, y: 0, width: W, height: H))
    NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: false)
    let font: [NSAttributedString.Key: Any] = [.font: NSFont.boldSystemFont(ofSize: 14), .foregroundColor: NSColor.black]

    for (si, style) in Style.allCases.enumerated() {
        let x = label + pad + si * (cellW + pad)
        let factor = upscale(frames[0], style: style, scale: scale).factor
        let note = factor == scale || style == .pixel ? "" : " (\(factor)× + nearest)"
        ("\(style.rawValue)\(note)" as NSString).draw(at: NSPoint(x: x, y: H - header + 6), withAttributes: font)
        for (fi, frame) in frames.enumerated() {
            let y = H - header - (fi + 1) * (cellH + pad)
            ctx.setFillColor(CGColor(red: 0.80, green: 0.84, blue: 0.88, alpha: 1))
            ctx.fill(CGRect(x: x, y: y, width: cellW, height: cellH))
            let up = upscale(frame, style: style, scale: scale)
            drawFrame(up.image, factor: up.factor, in: ctx,
                      anchorX: x / scale + 2 - box.minX, anchorY: y / scale + 2 + box.maxY, scale: scale)
            if si == 0 {
                ("\(picks[fi].0)" as NSString).draw(at: NSPoint(x: pad, y: y + cellH / 2), withAttributes: font)
            }
        }
    }
    let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: path) as CFURL, "public.png" as CFString, 1, nil)!
    CGImageDestinationAddImage(dest, ctx.makeImage()!, nil)
    guard CGImageDestinationFinalize(dest) else { throw SpriteError(description: "Couldn't write \(path)") }
}

// MARK: - Main

let args = CommandLine.arguments
if let i = args.firstIndex(of: "--contact-sheet"), i + 1 < args.count {
    do { try renderContactSheet(to: args[i + 1]) } catch { print("Error: \(error)"); exit(1) }
    exit(0)
}

if let i = args.firstIndex(of: "--compare"), i + 1 < args.count {
    let scale = args.firstIndex(of: "--scale").flatMap { $0 + 1 < args.count ? Int(args[$0 + 1]) : nil } ?? 4
    do { try renderComparison(to: args[i + 1], scale: scale) } catch { print("Error: \(error)"); exit(1) }
    exit(0)
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
