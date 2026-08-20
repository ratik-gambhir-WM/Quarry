import westMonroeLogo from "../../assets/Screenshot 2026-05-07 at 8.24.22 PM.png";

type BrandLockupProps = {
  subtitle: string;
  title: string;
};

export function BrandLockup({ subtitle, title }: BrandLockupProps) {
  return (
    <header className="flex flex-col items-center gap-md text-center">
      <img
        alt="West Monroe"
        className="h-10 w-auto object-contain"
        decoding="async"
        src={westMonroeLogo}
      />
      <div className="space-y-xs">
        <h1 className="type-h3 text-on-surface">{title}</h1>
        <p className="type-body-sm max-w-[28rem] text-secondary">{subtitle}</p>
      </div>
    </header>
  );
}
