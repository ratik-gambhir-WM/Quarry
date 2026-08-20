const footerLinks = ["Legal", "Privacy Policy", "Support", "Contact"];

export function AppFooter() {
  return (
    <footer className="border-t border-outline-variant/80 bg-white/70 px-gutter py-md backdrop-blur-xl">
      <div className="mx-auto flex w-full max-w-[1280px] flex-col items-center justify-between gap-sm md:flex-row">
        <span className="type-label text-secondary">© 2024 West Monroe Partners. All rights reserved.</span>
        <nav aria-label="Footer" className="flex flex-wrap items-center justify-center gap-lg">
          {footerLinks.map((link) => (
            <a
              className="type-label text-secondary transition-colors hover:text-primary"
              href="#"
              key={link}
            >
              {link}
            </a>
          ))}
        </nav>
      </div>
    </footer>
  );
}
