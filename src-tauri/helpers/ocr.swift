// Reads the text out of one image, a line per line of output.
//
// Apple's Vision does this well, locally and for free, but only from Swift --
// hence a helper binary rather than another crate. It is compiled by build.rs
// and embedded in the app, so there is nothing for anyone to install.

import Foundation
import Vision
import AppKit

let args = CommandLine.arguments

guard args.count > 1,
      let image = NSImage(contentsOfFile: args[1]),
      let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil)
else {
    FileHandle.standardError.write(Data("cannot read image\n".utf8))
    exit(1)
}

let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.usesLanguageCorrection = true

// Serbian is not a recognition language. Czech is, and shares our diacritics,
// which is why it reads "sledeća" where English gives "sledeéa".
if args.count > 2 {
    request.recognitionLanguages = Array(args[2...])
}

do {
    try VNImageRequestHandler(cgImage: cgImage, options: [:]).perform([request])
} catch {
    FileHandle.standardError.write(Data("\(error)\n".utf8))
    exit(1)
}

for observation in request.results ?? [] {
    if let best = observation.topCandidates(1).first {
        print(best.string)
    }
}
