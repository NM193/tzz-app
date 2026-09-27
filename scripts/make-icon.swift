// Draws the app icon and the menu bar icons.
//
// A design-time tool, not part of the app: run it when the mark changes, then
// `npx tauri icon assets/icon.png` turns the result into every size macOS
// wants. Written in Swift because the project already requires swiftc for the
// OCR helper, and CoreGraphics draws rounded caps exactly.
//
//   swiftc -O scripts/make-icon.swift -o /tmp/make-icon && /tmp/make-icon

import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// The mark, on a 100 x 100 grid: sound on the left, writing on the right.
struct Mark {
    /// Vertical bars: x, top, bottom.
    let bars: [(CGFloat, CGFloat, CGFloat)]
    /// Horizontal rules: y, left, right.
    let rules: [(CGFloat, CGFloat, CGFloat)]
    let weight: CGFloat
}

/// Full detail, for anything the size of a Dock icon or larger.
let full = Mark(
    bars: [(22, 38, 62), (34, 26, 74), (46, 42, 58)],
    rules: [(36, 60, 78), (50, 60, 78), (64, 60, 71)],
    weight: 6
)

/// Fewer strokes, drawn heavier: below about 32 points the full mark closes up.
let small = Mark(
    bars: [(26, 34, 66), (40, 24, 76)],
    rules: [(38, 58, 80), (62, 58, 72)],
    weight: 8
)

func context(_ size: Int) -> CGContext {
    let ctx = CGContext(
        data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    )!
    ctx.setAllowsAntialiasing(true)
    ctx.interpolationQuality = .high
    return ctx
}

/// Draw the mark into a square of `side`, offset by `origin`, in `colour`.
func draw(_ mark: Mark, into ctx: CGContext, side: CGFloat, origin: CGPoint, colour: CGColor) {
    let unit = side / 100
    ctx.saveGState()
    ctx.setStrokeColor(colour)
    ctx.setLineCap(.round)
    ctx.setLineWidth(mark.weight * unit)

    // The grid runs top-down; CoreGraphics runs bottom-up.
    func point(_ x: CGFloat, _ y: CGFloat) -> CGPoint {
        CGPoint(x: origin.x + x * unit, y: origin.y + (100 - y) * unit)
    }

    for (x, top, bottom) in mark.bars {
        ctx.move(to: point(x, top))
        ctx.addLine(to: point(x, bottom))
    }
    for (y, left, right) in mark.rules {
        ctx.move(to: point(left, y))
        ctx.addLine(to: point(right, y))
    }
    ctx.strokePath()
    ctx.restoreGState()
}

func write(_ ctx: CGContext, to path: String) {
    let url = URL(fileURLWithPath: path)
    guard let image = ctx.makeImage(),
          let out = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)
    else {
        FileHandle.standardError.write("could not write \(path)\n".data(using: .utf8)!)
        exit(1)
    }
    CGImageDestinationAddImage(out, image, nil)
    CGImageDestinationFinalize(out)
    print("wrote \(path)")
}

func rgb(_ hex: UInt32, alpha: CGFloat = 1) -> CGColor {
    CGColor(
        red: CGFloat((hex >> 16) & 0xff) / 255,
        green: CGFloat((hex >> 8) & 0xff) / 255,
        blue: CGFloat(hex & 0xff) / 255,
        alpha: alpha
    )
}

/// The app icon: a squircle inset in the canvas, the way macOS icons are.
func appIcon(path: String, tile: CGColor, ink: CGColor) {
    let size = 1024
    let ctx = context(size)
    let canvas = CGFloat(size)

    // macOS leaves a margin around the tile; 82% of the canvas is the shape.
    let side = canvas * 0.82
    let inset = (canvas - side) / 2
    let radius = side * 0.2237 // the squircle's corner, near enough

    let rect = CGRect(x: inset, y: inset, width: side, height: side)
    ctx.setFillColor(tile)
    ctx.addPath(CGPath(roundedRect: rect, cornerWidth: radius, cornerHeight: radius, transform: nil))
    ctx.fillPath()

    draw(full, into: ctx, side: side, origin: CGPoint(x: inset, y: inset), colour: ink)
    write(ctx, to: path)
}

/// A menu bar icon: black on nothing, because macOS recolours a template.
func trayIcon(path: String, recording: Bool) {
    let size = 44
    let ctx = context(size)
    let side = CGFloat(size)
    let black = rgb(0x000000)

    if recording {
        // A filled dot is the one shape that still says "recording" at 18 points.
        let d = side * 0.46
        ctx.setFillColor(black)
        ctx.fillEllipse(in: CGRect(x: (side - d) / 2, y: (side - d) / 2, width: d, height: d))
    } else {
        draw(small, into: ctx, side: side, origin: .zero, colour: black)
    }
    write(ctx, to: path)
}

let bone = rgb(0xFFFDDC)
let dark = rgb(0x100E0E)
let amber = rgb(0xF7AB29)

appIcon(path: "assets/icon.png", tile: dark, ink: bone)
appIcon(path: "assets/icon-amber.png", tile: amber, ink: dark)
trayIcon(path: "src-tauri/icons/tray-idle.png", recording: false)
trayIcon(path: "src-tauri/icons/tray-recording.png", recording: true)
