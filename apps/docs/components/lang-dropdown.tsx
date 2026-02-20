'use client';

import { usePathname, useRouter } from 'next/navigation';
import { Languages, ChevronDown } from 'lucide-react';
import { useRef, useState, useEffect } from 'react';

const locales = [
  { locale: 'en', label: 'English' },
  { locale: 'cn', label: '中文' },
];

function resolveHref(locale: string, pathname: string): string {
  const stripped = pathname.replace(/^\/(cn)/, '') || '/';
  return locale === 'en' ? stripped : `/${locale}${stripped}`;
}

export function LangDropdown() {
  const pathname = usePathname();
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  // Detect current locale from pathname
  const current = pathname.startsWith('/cn') ? 'cn' : 'en';
  const currentLabel = locales.find((l) => l.locale === current)?.label ?? 'English';

  // Close on outside click
  useEffect(() => {
    function handler(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label="Switch language"
        className="flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm font-medium text-slate-600 transition-colors hover:border-indigo-300 hover:bg-indigo-50 hover:text-indigo-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2"
      >
        <Languages className="h-4 w-4" />
        <span>{currentLabel}</span>
        <ChevronDown
          className={`h-3.5 w-3.5 transition-transform duration-150 ${open ? 'rotate-180' : ''}`}
        />
      </button>

      {open && (
        <ul
          role="listbox"
          aria-label="Language"
          className="absolute right-0 top-full z-50 mt-1.5 min-w-[120px] overflow-hidden rounded-xl border border-slate-200 bg-white py-1 shadow-lg"
          style={{ boxShadow: 'var(--ct-shadow-hover)' }}
        >
          {locales.map(({ locale, label }) => {
            const isActive = locale === current;
            return (
              <li key={locale} role="option" aria-selected={isActive}>
                <button
                  type="button"
                  onClick={() => {
                    setOpen(false);
                    if (!isActive) router.push(resolveHref(locale, pathname));
                  }}
                  className={`flex w-full items-center gap-2 px-4 py-2.5 text-sm transition-colors ${
                    isActive
                      ? 'bg-indigo-50 font-semibold text-indigo-700'
                      : 'text-slate-600 hover:bg-slate-50 hover:text-slate-900'
                  }`}
                >
                  {isActive && <span className="h-1.5 w-1.5 rounded-full bg-indigo-500" />}
                  {!isActive && <span className="h-1.5 w-1.5" />}
                  {label}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
