type WorkspaceHeaderProps = {
  title: string;
};

export function WorkspaceHeader({ title }: WorkspaceHeaderProps) {
  return (
    <header>
      <h1 className="text-[20px] font-semibold leading-6 tracking-[-0.015em] text-text-main [font-family:var(--font-heading)]">
        {title}
      </h1>
    </header>
  );
}
