import westMonroeLogo from "../../assets/Screenshot 2026-05-07 at 8.24.22 PM.png";

type BrandLockupProps = {
  subtitle: string;
  title: string;
  compact?: boolean;
};

export function BrandLockup({
  subtitle,
  title,
  compact = false,
}: BrandLockupProps) {
  return (
    <header
      className={`flex items-center gap-md text-center ${compact ? "flex-col sm:flex-row sm:justify-between sm:text-left" : "flex-col"}`}
    >
      <img
        alt="West Monroe"
        className={`${compact ? "h-8" : "h-10"} w-auto object-contain`}
        decoding="async"
        src={westMonroeLogo}
      />
      <div className="space-y-xs">
        <h1 className="type-h3 text-on-surface">{title}</h1>
        <p className="type-body-sm max-w-[28rem] text-on-surface-variant">
          {subtitle}
        </p>
      </div>
    </header>
  );
}
