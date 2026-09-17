import type { ReactNode } from "react";

type Props = {
  kicker: string;
  title: ReactNode;
  lead: string;
  art?: ReactNode;
};

/** The top of every screen: what this screen does, in two lines. */
export function Hero({ kicker, title, lead, art }: Props) {
  return (
    <header className="hero">
      <div className="hero__text">
        <span className="hero__kicker">{kicker}</span>
        <h1 className="hero__title">{title}</h1>
        <p className="hero__lead">{lead}</p>
      </div>
      {art && <div className="hero__art">{art}</div>}
    </header>
  );
}

/** Two frosted tiles: a video going in, a document coming out. */
export function ConvertArt() {
  return (
    <svg viewBox="0 0 260 150" width="260" height="150" aria-hidden="true" className="art">
      <defs>
        <linearGradient id="tile" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--art-hi)" />
          <stop offset="1" stopColor="var(--art-lo)" />
        </linearGradient>
        <linearGradient id="tileB" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--art-hi)" />
          <stop offset="1" stopColor="var(--art-accent)" />
        </linearGradient>
        <filter id="soft" x="-20%" y="-20%" width="140%" height="140%">
          <feGaussianBlur stdDeviation="6" />
        </filter>
      </defs>

      {/* shadows */}
      <rect x="28" y="52" width="86" height="86" rx="18" fill="var(--art-shadow)" filter="url(#soft)" />
      <rect x="152" y="58" width="86" height="86" rx="18" fill="var(--art-shadow)" filter="url(#soft)" />

      {/* input tile: a video file */}
      <g transform="translate(20 20) rotate(-6 43 43)">
        <rect width="86" height="86" rx="18" fill="url(#tile)" stroke="var(--art-edge)" />
        <path
          d="M30 24h20l8 8v30a4 4 0 0 1-4 4H30a4 4 0 0 1-4-4V28a4 4 0 0 1 4-4z"
          fill="none"
          stroke="var(--art-ink)"
          strokeWidth="2.5"
          strokeLinejoin="round"
        />
        <path d="M50 24v8h8" fill="none" stroke="var(--art-ink)" strokeWidth="2.5" strokeLinejoin="round" />
        <path d="M37 42v12l10-6z" fill="var(--art-ink)" />
      </g>

      {/* arrow */}
      <path
        d="M116 78h24"
        stroke="var(--art-accent)"
        strokeWidth="3"
        strokeLinecap="round"
      />
      <path d="m134 70 8 8-8 8" fill="none" stroke="var(--art-accent)" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />

      {/* output tile: a document with lines */}
      <g transform="translate(152 30) rotate(6 43 43)">
        <rect width="86" height="86" rx="18" fill="url(#tileB)" stroke="var(--art-edge)" />
        <path
          d="M30 22h20l8 8v32a4 4 0 0 1-4 4H30a4 4 0 0 1-4-4V26a4 4 0 0 1 4-4z"
          fill="none"
          stroke="var(--art-ink)"
          strokeWidth="2.5"
          strokeLinejoin="round"
        />
        <path d="M50 22v8h8" fill="none" stroke="var(--art-ink)" strokeWidth="2.5" strokeLinejoin="round" />
        <path d="M33 42h20M33 49h20M33 56h12" stroke="var(--art-ink)" strokeWidth="2.5" strokeLinecap="round" />
      </g>
    </svg>
  );
}

/** A single tile with a microphone, and sound rings when live. */
export function RecordArt({ live }: { live: boolean }) {
  return (
    <svg viewBox="0 0 200 150" width="200" height="150" aria-hidden="true" className="art">
      <defs>
        <linearGradient id="tileR" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="var(--art-hi)" />
          <stop offset="1" stopColor={live ? "var(--art-live)" : "var(--art-lo)"} />
        </linearGradient>
        <filter id="softR" x="-20%" y="-20%" width="140%" height="140%">
          <feGaussianBlur stdDeviation="6" />
        </filter>
      </defs>
      <rect x="62" y="48" width="86" height="86" rx="18" fill="var(--art-shadow)" filter="url(#softR)" />
      <g transform="translate(57 22) rotate(-4 43 43)">
        <rect width="86" height="86" rx="18" fill="url(#tileR)" stroke="var(--art-edge)" />
        <rect x="35" y="20" width="16" height="30" rx="8" fill="none" stroke="var(--art-ink)" strokeWidth="2.5" />
        <path d="M27 42a16 16 0 0 0 32 0M43 58v8" fill="none" stroke="var(--art-ink)" strokeWidth="2.5" strokeLinecap="round" />
      </g>
      {live && (
        <g fill="none" stroke="var(--art-live)" strokeWidth="2" strokeLinecap="round" className="art__rings">
          <path d="M28 50a40 40 0 0 0 0 50" />
          <path d="M16 40a56 56 0 0 0 0 70" />
          <path d="M172 50a40 40 0 0 1 0 50" />
          <path d="M184 40a56 56 0 0 1 0 70" />
        </g>
      )}
    </svg>
  );
}
