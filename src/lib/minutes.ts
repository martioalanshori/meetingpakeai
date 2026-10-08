// Notulen sebagai teks (PRD §14.7): dipakai tombol Salin dan Ekspor.
// markdown = format §14.7; text = tanpa sintaks markdown; whatsapp = *tebal* dan bullet "•".
import { formatDateTime, formatDuration, formatTime, formatTimestamp } from "./format";
import { id as t } from "./i18n/id";
import type { ActionItem, MeetingDetail, TranscriptSegment } from "./types";

export type MinutesStyle = "markdown" | "text" | "whatsapp";

function heading(text: string, style: MinutesStyle): string {
  if (style === "markdown") return `## ${text}`;
  if (style === "whatsapp") return `*${text}*`;
  return text;
}

function bullet(style: MinutesStyle): string {
  return style === "whatsapp" ? "• " : "- ";
}

function dateLine(m: MeetingDetail, style: MinutesStyle): string {
  let when = formatDateTime(m.startedAt).replace(/ (\d\d\.\d\d)$/, ", $1");
  if (m.endedAt) when += `–${formatTime(m.endedAt)}`;
  if (m.durationMs > 0) when += ` (${formatDuration(m.durationMs)})`;
  const label = `${t.minutes.date}:`;
  if (style === "markdown") return `**${label}** ${when}`;
  if (style === "whatsapp") return `*${label}* ${when}`;
  return `${label} ${when}`;
}

export function formatActionItem(a: ActionItem, style: MinutesStyle): string {
  let line = a.task;
  if (a.assignee) line += ` — ${t.detail.assignee} ${a.assignee}`;
  if (a.due) line += ` — ${t.detail.due} ${a.due}`;
  if (style === "markdown") return `- [${a.done ? "x" : " "}] ${line}`;
  if (style === "whatsapp") return `${a.done ? "✅" : "☐"} ${line}`;
  return `- ${line}`;
}

function titleLine(m: MeetingDetail, style: MinutesStyle): string {
  return style === "markdown" ? `# ${m.title}` : style === "whatsapp" ? `*${m.title}*` : m.title;
}

/** Tab Ringkasan: judul, tanggal, ringkasan, keputusan, topik. */
export function formatSummaryTab(m: MeetingDetail, style: MinutesStyle): string {
  const out = [titleLine(m, style), dateLine(m, style), ""];
  const s = m.summary;
  out.push(heading(t.detail.summary, style));
  out.push(s?.status === "ok" && s.summary ? s.summary : s?.status === "empty" ? t.summary.noSpeech : "—", "");
  out.push(heading(t.detail.decisions, style));
  if (!s || s.decisions.length === 0) out.push(t.detail.noDecisions);
  else out.push(...s.decisions.map((d) => bullet(style) + d));
  if (s && s.topics.length > 0) out.push("", heading(t.detail.topics, style), s.topics.join(", "));
  return out.join("\n").trimEnd() + "\n";
}

/** Tab Action Items: judul meeting lalu daftar tugas. */
export function formatActionItems(m: MeetingDetail, style: MinutesStyle): string {
  const lines = [heading(t.minutes.actionItemsOf(m.title), style), ""];
  if (m.actionItems.length === 0) lines.push(t.detail.noActionItems);
  else lines.push(...m.actionItems.map((a) => formatActionItem(a, style)));
  return lines.join("\n") + "\n";
}

/** Tab Transkrip: satu baris per segment `[HH:MM:SS] Label: teks`. */
export function formatTranscript(m: MeetingDetail, transcript: TranscriptSegment[]): string {
  const lines = [m.title, dateLine(m, "text"), ""];
  for (const seg of transcript) {
    const who = seg.channel === "mic" ? m.labels.mic : m.labels.system;
    lines.push(`[${formatTimestamp(seg.startMs)}] ${who}: ${seg.text}`);
  }
  return lines.join("\n") + "\n";
}

/** Notulen lengkap; transkrip hanya disertakan jika diberikan (ekspor). */
export function formatMinutes(m: MeetingDetail, style: MinutesStyle, transcript?: TranscriptSegment[]): string {
  const out: string[] = [];
  out.push(style === "markdown" ? `# ${m.title}` : style === "whatsapp" ? `*${m.title}*` : m.title, "");
  out.push(dateLine(m, style), "");

  const s = m.summary;
  out.push(heading(t.detail.summary, style));
  out.push(s?.status === "ok" && s.summary ? s.summary : s?.status === "empty" ? t.summary.noSpeech : "—", "");

  out.push(heading(t.detail.decisions, style));
  if (!s || s.decisions.length === 0) out.push(t.detail.noDecisions);
  else out.push(...s.decisions.map((d) => bullet(style) + d));
  out.push("");

  out.push(heading(t.detail.tabActionItems, style));
  if (m.actionItems.length === 0) out.push(t.detail.noActionItems);
  else out.push(...m.actionItems.map((a) => formatActionItem(a, style)));
  out.push("");

  if (s && s.topics.length > 0) {
    out.push(heading(t.detail.topics, style), s.topics.join(", "), "");
  }

  if (transcript && transcript.length > 0) {
    out.push(heading(t.detail.tabTranscript, style));
    for (const seg of transcript) {
      const who = seg.channel === "mic" ? m.labels.mic : m.labels.system;
      out.push(`[${formatTimestamp(seg.startMs)}] ${who}: ${seg.text}`);
    }
    out.push("");
  }
  return out.join("\n").trimEnd() + "\n";
}
