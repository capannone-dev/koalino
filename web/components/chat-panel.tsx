// A chat with notes (#97): on a meeting's page (that meeting), a project's page (that project)
// and the Chat page (every meeting, summary and project). Answers come from the default AI
// connection and say what the model was given; the chat is kept by the server and can be deleted.
// The places an answer cites are listed under it as links (#110).

"use client";

import Link from "next/link";
import { useEffect, useId, useState, type FormEvent } from "react";
import {
  askChat,
  chatErrorMessage,
  clearChat,
  getChat,
  sourceHref,
  sourceLabel,
  type ChatMessage,
  type ChatTopic
} from "@/lib/chat-api";

const buttonClass =
  "inline-flex min-h-11 items-center justify-center rounded-lg border border-line-strong px-4 hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus disabled:text-faint";
const fieldClass =
  "min-h-11 w-full rounded-lg border border-line-strong bg-surface px-3 py-2 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30";

export type ChatPanelProps = {
  topic: ChatTopic;
  /** The heading, such as "Chat about this meeting". */
  heading: string;
};

/**
 * Shows a chat and asks questions.
 *
 * Inputs: see `ChatPanelProps`. Outputs: the messages, a question field and a status line.
 * Side effects: reads the chat on mount; asking and deleting call the daemon.
 */
export function ChatPanel({ topic, heading }: ChatPanelProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [question, setQuestion] = useState("");
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const fieldId = useId();
  const topicKey = topic.scope === "all" ? "all" : `${topic.scope}:${topic.id}`;

  useEffect(() => {
    let current = true;
    getChat(topic)
      .then((loaded) => {
        if (current) {
          setMessages(loaded);
        }
      })
      .catch((error: unknown) => {
        if (current) {
          setStatus(chatErrorMessage(error));
        }
      });
    return () => {
      current = false;
    };
    // The topic is identified by its key.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [topicKey]);

  async function ask(event: FormEvent) {
    event.preventDefault();
    if (!question.trim()) {
      setStatus("Type a question.");
      return;
    }
    setBusy(true);
    setStatus("Waiting for the answer…");
    try {
      const kept = await askChat(topic, question);
      setMessages((earlier) => [...earlier, kept.question, kept.answer]);
      setQuestion("");
      setStatus(null);
    } catch (error) {
      setStatus(chatErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function clear() {
    if (!window.confirm("Delete this chat? Its questions and answers are deleted from the server.")) {
      return;
    }
    try {
      await clearChat(topic);
      setMessages([]);
      setStatus("Chat deleted.");
    } catch (error) {
      setStatus(chatErrorMessage(error));
    }
  }

  return (
    <section className="card space-y-3" aria-label={heading}>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="text-xl font-semibold">{heading}</h2>
        {messages.length > 0 ? (
          <button type="button" className={buttonClass} disabled={busy} onClick={() => void clear()}>
            Delete chat
          </button>
        ) : null}
      </div>
      {messages.length === 0 ? (
        <p className="text-sm text-muted">
          Ask about your notes. The answer comes from your default AI connection, which is sent the notes that fit.
        </p>
      ) : (
        <ol className="space-y-3">
          {messages.map((message, index) => (
            <li
              key={index}
              className={
                message.role === "user"
                  ? "ml-auto max-w-[85%] rounded-lg bg-surface-muted px-3 py-2"
                  : "max-w-[85%] rounded-lg border border-line px-3 py-2"
              }
            >
              <p className="sr-only">{message.role === "user" ? "You asked:" : "Answer:"}</p>
              <div className="whitespace-pre-wrap break-words">{message.text}</div>
              {message.role === "assistant" && message.sources && message.sources.length > 0 ? (
                <ul className="mt-2 flex flex-wrap gap-2" aria-label="Sources">
                  {message.sources.map((source) => {
                    const href = sourceHref(source);
                    return (
                      <li key={source.number} className="rounded-full border border-line px-3 py-1 text-sm">
                        {href ? (
                          <Link href={href} className="text-accent-text underline-offset-2 hover:underline">
                            {sourceLabel(source)}
                          </Link>
                        ) : (
                          sourceLabel(source)
                        )}
                      </li>
                    );
                  })}
                </ul>
              ) : null}
              {message.role === "assistant" && message.included.length > 0 ? (
                <p className="mt-2 text-sm text-muted">Given: {message.included.join("; ")}</p>
              ) : null}
              {message.role === "assistant" && message.omitted.length > 0 ? (
                <p className="text-sm text-muted">Left out for length: {message.omitted.join("; ")}</p>
              ) : null}
              {message.role === "assistant" && message.connection ? (
                <p className="text-sm text-muted">Answered by {message.connection}</p>
              ) : null}
            </li>
          ))}
        </ol>
      )}
      <form className="flex flex-wrap items-end gap-2" onSubmit={(event) => void ask(event)}>
        <div className="min-w-60 flex-1">
          <label htmlFor={fieldId} className="block text-sm font-semibold">
            Your question
          </label>
          <textarea
            id={fieldId}
            rows={2}
            className={fieldClass}
            value={question}
            disabled={busy}
            onChange={(event) => setQuestion(event.target.value)}
          />
        </div>
        <button type="submit" className={buttonClass} disabled={busy}>
          Ask
        </button>
      </form>
      <p aria-live="polite" className="text-sm text-muted">
        {status}
      </p>
    </section>
  );
}
