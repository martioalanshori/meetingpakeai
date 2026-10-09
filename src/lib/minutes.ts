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
  if (m.endedAt) when += ` – ${formatTime(m.endedAt)}`;
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
  if (s && s.keyPoints.length > 0) out.push(heading(t.detail.keyPoints, style), ...s.keyPoints.map((p) => bullet(style) + p), "");
  out.push(heading(t.detail.summary, style));
  out.push(s?.status === "ok" && s.summary ? s.summary : s?.status === "empty" ? t.summary.noSpeech : "—", "");
  out.push(heading(t.detail.decisions, style));
  if (!s || s.decisions.length === 0) out.push(t.detail.noDecisions);
  else out.push(...s.decisions.map((d) => bullet(style) + d));
  if (s && s.openQuestions.length > 0)
    out.push("", heading(t.detail.openQuestions, style), ...s.openQuestions.map((q) => bullet(style) + q));
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

/** Tab Transkrip: satu baris per segment `[HH:MM:SS] teks`. */
export function formatTranscript(m: MeetingDetail, transcript: TranscriptSegment[]): string {
  const lines = [m.title, dateLine(m, "text"), ""];
  for (const seg of transcript) {
    lines.push(`[${formatTimestamp(seg.startMs)}] ${seg.text}`);
  }
  return lines.join("\n") + "\n";
}

/** Notulen lengkap; transkrip hanya disertakan jika diberikan (ekspor). */
export function formatMinutes(m: MeetingDetail, style: MinutesStyle, transcript?: TranscriptSegment[]): string {
  const out: string[] = [];
  out.push(style === "markdown" ? `# ${m.title}` : style === "whatsapp" ? `*${m.title}*` : m.title, "");
  out.push(dateLine(m, style), "");

  const s = m.summary;
  if (s && s.keyPoints.length > 0) out.push(heading(t.detail.keyPoints, style), ...s.keyPoints.map((p) => bullet(style) + p), "");
  out.push(heading(t.detail.summary, style));
  out.push(s?.status === "ok" && s.summary ? s.summary : s?.status === "empty" ? t.summary.noSpeech : "—", "");

  out.push(heading(t.detail.decisions, style));
  if (!s || s.decisions.length === 0) out.push(t.detail.noDecisions);
  else out.push(...s.decisions.map((d) => bullet(style) + d));
  out.push("");

  if (s && s.openQuestions.length > 0) {
    out.push(heading(t.detail.openQuestions, style), ...s.openQuestions.map((q) => bullet(style) + q), "");
  }

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
      out.push(`[${formatTimestamp(seg.startMs)}] ${seg.text}`);
    }
    out.push("");
  }
  return out.join("\n").trimEnd() + "\n";
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function inline(s: string): string {
  return escapeHtml(s).replace(/\*\*(.+?)\*\*/g, "<b>$1</b>");
}

/** Teks berformat markdown sederhana (hasil `style = "markdown"`) → HTML untuk ditempel ke email/Docs/Word. */
export function markdownToHtml(md: string): string {
  const out: string[] = [];
  let list: string[] = [];
  const flush = () => {
    if (list.length > 0) out.push(`<ul>${list.join("")}</ul>`);
    list = [];
  };
  for (const raw of md.split("\n")) {
    const line = raw.trimEnd();
    const item = /^\s*[-•]\s+(?:\[( |x)\]\s+)?(.*)$/.exec(line);
    if (item) {
      const mark = item[1] === "x" ? "☑ " : item[1] === " " ? "☐ " : "";
      list.push(`<li>${mark}${inline(item[2])}</li>`);
      continue;
    }
    flush();
    if (line === "") continue;
    if (line.startsWith("# ")) out.push(`<h2>${inline(line.slice(2))}</h2>`);
    else if (line.startsWith("## ")) out.push(`<h3>${inline(line.slice(3))}</h3>`);
    else out.push(`<p>${inline(line)}</p>`);
  }
  flush();
  return out.join("");
}

/** Teks polos dengan daftar "- " → HTML (paragraf + daftar berpoin). */
export function textToHtml(text: string): string {
  return markdownToHtml(text.replace(/^#+ /gm, ""));
}
