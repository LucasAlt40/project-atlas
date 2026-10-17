/** One statement the agent has already finished writing in its structured answer. */
export interface NarratedFinding {
  category: string;
  key: string;
  value: string;
}

export interface Narration {
  /** What the agent said in words, before (or around) the structured answer. */
  prose: string[];
  /** The findings completed so far, in the order they were written. */
  findings: NarratedFinding[];
  /** The agent has started writing the structured answer (a JSON object). */
  building: boolean;
}

const FINDING =
  /\{\s*"category"\s*:\s*"((?:[^"\\]|\\.)*)"\s*,\s*"key"\s*:\s*"((?:[^"\\]|\\.)*)"\s*,\s*"value"\s*:\s*"((?:[^"\\]|\\.)*)"/g;

function unescape(text: string): string {
  try {
    return JSON.parse(`"${text}"`) as string;
  } catch {
    return text;
  }
}

/**
 * What an agent's streamed output means to a person. The agent answers in one JSON object, so the
 * raw stream is code cut at an arbitrary point: here it is read back into the words it said and
 * the findings it finished, and the half-written rest is left out.
 */
export function readNarration(raw: string): Narration {
  const start = raw.indexOf('{');
  const before = start < 0 ? raw : raw.slice(0, start);
  const json = start < 0 ? '' : raw.slice(start);
  // A code fence the agent opened before the object is not something to read.
  const prose = before
    .replace(/```(?:json)?/g, '')
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line !== '');
  const findings = [...json.matchAll(FINDING)].map((m) => ({
    category: unescape(m[1] ?? ''),
    key: unescape(m[2] ?? ''),
    value: unescape(m[3] ?? ''),
  }));
  return { prose, findings, building: start >= 0 };
}
