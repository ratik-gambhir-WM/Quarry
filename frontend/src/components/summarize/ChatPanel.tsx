import { Icon } from "../ui/Icon";

export function ChatPanel() {
  return (
    <section className="flex min-h-[calc(100vh-210px)] flex-col items-center justify-center px-4 pb-20 pt-14">
      <div className="mb-8 flex items-center gap-4">
        <div className="flex h-10 w-10 items-center justify-center rounded-full bg-primary/10 text-primary">
          <Icon className="h-5 w-5" name="sparkles" />
        </div>
        <h2 className="type-h1 text-text-main">Ask Quarry</h2>
      </div>

      <div className="w-full max-w-[640px] rounded-[19px] border border-white/85 bg-white/82 p-4 shadow-[0_18px_48px_rgba(7,1,84,0.08)] backdrop-blur-md">
        <textarea
          className="min-h-[86px] w-full resize-none bg-transparent px-2 py-2 text-[16px] text-text-main outline-none placeholder:text-muted"
          placeholder="Type a message..."
        />
        <div className="mt-2 flex items-center justify-between gap-4">
          <button
            aria-label="Add context"
            className="flex h-10 w-10 items-center justify-center rounded-full text-primary transition hover:bg-primary/8"
            type="button"
          >
            <Icon className="h-5 w-5" name="plus" />
          </button>
          <div className="flex items-center gap-3">
            <span className="rounded-full bg-primary/8 px-4 py-2 text-[13px] font-semibold text-primary">Quarry</span>
            <button
              aria-label="Send message"
              className="flex h-10 w-10 items-center justify-center rounded-full bg-action text-on-action transition enabled:hover:bg-action-hover"
              type="button"
            >
              <Icon className="h-5 w-5" name="send" />
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}
