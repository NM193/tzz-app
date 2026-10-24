/**
 * What the language dialog offers, and what each choice sends to Whisper.
 *
 * Whisper cannot tell Serbian from Croatian or Bosnian -- the owner's own
 * Serbian came back as "hr" -- and `-l sr` is the one that writes ekavian, so
 * all three are Serbian here and send "sr".
 */

const SERBIAN = ["sr", "hr", "bs"];
const ALWAYS = ["sr", "en"];

/** The code a guess selects: Serbian for any of the three, else as heard. */
export function choiceFor(code: string): string {
  return SERBIAN.includes(code) ? "sr" : code;
}

/** Serbian and English always; a third button for anything else Whisper names. */
export function choicesFor(detected: string | null): string[] {
  if (!detected) return ALWAYS;
  const choice = choiceFor(detected);
  return ALWAYS.includes(choice) ? ALWAYS : [...ALWAYS, choice];
}

/** "hr" -> "Croatian". The code itself when the name is unknown. */
export function languageName(code: string): string {
  try {
    return new Intl.DisplayNames(["en"], { type: "language" }).of(code) ?? code;
  } catch {
    return code;
  }
}

/** "Detected · Croatian 53%" -- uppercased by the stylesheet, not here. */
export function detectedLabel(detected: { code: string; probability: number }): string {
  return `Detected · ${languageName(detected.code)} ${Math.round(detected.probability * 100)}%`;
}
