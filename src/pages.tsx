import { DangerZone } from "./components/DangerZone";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "./components/ui/tabs";
import { TransactionOptionsSettings } from "./components/TransactionOptionsSettings";
import { TransactionShortcut, AddTransactionButton } from "./components/TransactionShortcut";
import { SidebarAccounts } from "./components/SidebarAccounts";
import { SidebarResizeHandle } from "./components/SidebarResizeHandle";
import { toast } from "sonner";
import { Toaster } from "./components/ui/sonner";
import { CurrencySetting } from "./components/CurrencySetting";
import {
  Fragment,
  createContext,
  useContext,
  useEffect,
  useState,
  type CSSProperties,
} from "react";
import { Link, Outlet, useRouterState } from "@tanstack/react-router";
import {
  ArrowLeftRight,
  ChartNoAxesCombined,
  CalendarDays,
  ListOrdered,
  Repeat,
  ArrowUpRight,
  Home,
  PanelLeftClose,
  PanelLeftOpen,
  Settings as SettingsIcon,
  Wallet,
} from "lucide-react";
import { getVersion } from "@tauri-apps/api/app";
import appConfig from "../src-tauri/tauri.conf.json";
import { periodLabel } from "./lib/period";
import {
  desktopAvailable,
  getSettings,
  updatePeriod,
  type Settings,
} from "./lib/desktop";

function readPreference(key: string, fallback: string) {
  try {
    return localStorage.getItem(key) ?? fallback;
  } catch {
    return fallback;
  }
}

function savePreference(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* Preferences are optional. */
  }
}

const navigation = [
  { to: "/", label: "Home", icon: Home },
  { to: "/outlook", label: "Outlook", icon: CalendarDays },
  { to: "/net-worth", label: "Net Worth", icon: ChartNoAxesCombined },
  { to: "/transactions", label: "Transactions", icon: ArrowLeftRight },
  { to: "/installments", label: "Installments", icon: ListOrdered },
  { to: "/subscriptions", label: "Subscriptions", icon: Repeat },
  { to: "/income", label: "Income", icon: ArrowUpRight },
  { to: "/accounts", label: "Accounts", icon: Wallet },
  { to: "/settings", label: "Settings", icon: SettingsIcon },
] as const;

const ThemeContext = createContext({
  dark: false,
  setDark: (_dark: boolean) => {},
});

export function AppLayout() {
  const [collapsed, setCollapsed] = useState(
    () => readPreference("sidebar-collapsed", "true") === "true",
  );
  const [sidebarWidth, setSidebarWidth] = useState(() => {
    const saved = Number(readPreference("sidebar-width", "184"));
    return Number.isFinite(saved) ? Math.min(360, Math.max(180, saved)) : 184;
  });
  const [windowWidth, setWindowWidth] = useState(window.innerWidth);
  const maxSidebarWidth = Math.max(180, Math.min(360, windowWidth - 420));
  const actualSidebarWidth = Math.min(sidebarWidth, maxSidebarWidth);
  useEffect(() => {
    const resize = () => setWindowWidth(window.innerWidth);
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, []);
  useEffect(() => {
    savePreference("sidebar-width", String(sidebarWidth));
  }, [sidebarWidth]);
  const [dark, setDark] = useState(
    () =>
      readPreference(
        "theme",
        matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light",
      ) === "dark",
  );
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const title =
    navigation.find((item) => item.to === pathname)?.label ?? "Heyday";

  useEffect(() => {
    savePreference("sidebar-collapsed", String(collapsed));
  }, [collapsed]);
  useEffect(() => {
    document.documentElement.dataset.theme = dark ? "dark" : "light";
    savePreference("theme", dark ? "dark" : "light");
  }, [dark]);

  return (
    <ThemeContext.Provider value={{ dark, setDark }}>
      <TransactionShortcut>
      <div
        className="grid h-dvh w-full grid-cols-[var(--sidebar-width)_minmax(0,1fr)] grid-rows-[minmax(0,1fr)] overflow-hidden"
        style={
          {
            "--sidebar-width": `${collapsed ? 48 : actualSidebarWidth}px`,
          } as CSSProperties
        }
      >
        <div className="relative min-h-0 min-w-0">
          <aside
            className={`flex h-full min-h-0 min-w-0 flex-col overflow-x-hidden overflow-y-auto border-r border-line bg-card pt-[18px] pb-3 [&>*]:min-w-0 [&>*]:max-w-full ${collapsed ? "px-1" : "px-2.5"}`}
          >
            <Link
              to="/"
              className={`mb-[26px] flex items-center gap-[9px] text-[16px] font-[750] tracking-[-.8px] whitespace-nowrap max-[650px]:text-[14px] ${collapsed ? "w-full justify-center" : ""}`}
              aria-label="Heyday Money home"
            >
              <img
                className="shrink-0"
                src="/logo.svg"
                alt=""
                width="30"
                height="30"
              />
              {!collapsed && (
                <span>
                  heyday<span className="font-normal text-muted">.money</span>
                </span>
              )}
            </Link>
            <nav
              className="flex flex-1 flex-col gap-1.5"
              aria-label="Main navigation"
            >
              {navigation.map(({ to, label, icon: Icon }) => (
                <Fragment key={to}>
                  <Link
                    to={to}
                    activeOptions={{ exact: true }}
                    activeProps={{ "aria-current": "page" }}
                    className={`flex min-w-0 max-w-full items-center gap-2.5 rounded-[9px] p-2.5 text-[14px] font-[550] text-muted hover:bg-page hover:text-ink aria-[current=page]:bg-soft aria-[current=page]:text-brand ${to === "/settings" ? "mt-auto" : ""} ${collapsed ? "justify-center" : ""}`}
                    title={collapsed ? label : undefined}
                    aria-label={label}
                  >
                    <Icon className="shrink-0" size={18} aria-hidden="true" />
                    {!collapsed && <span>{label}</span>}
                  </Link>
                  {to === "/accounts" && !collapsed && <SidebarAccounts />}
                </Fragment>
              ))}
            </nav>
            <button
              className={`mt-3 flex items-center gap-2.5 rounded-[9px] border-0 bg-transparent p-2.5 text-[12px] font-[550] text-muted hover:bg-page hover:text-ink ${collapsed ? "justify-center" : ""}`}
              onClick={() => setCollapsed((value) => !value)}
              aria-expanded={!collapsed}
              aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
              title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            >
              {collapsed ? (
                <PanelLeftOpen className="shrink-0" size={18} />
              ) : (
                <>
                  <PanelLeftClose className="shrink-0" size={18} />
                  <span>Collapse sidebar</span>
                </>
              )}
            </button>
          </aside>
          {!collapsed && (
            <SidebarResizeHandle
              width={actualSidebarWidth}
              maxWidth={maxSidebarWidth}
              onResize={setSidebarWidth}
            />
          )}
        </div>
        <div className="flex min-h-0 min-w-0 flex-col overflow-hidden">
          <header className="flex h-16 shrink-0 items-center justify-between border-b border-line px-7 max-[650px]:px-[18px]">
            <h1 className="text-[18px] font-[650]">{title}</h1>
            <AddTransactionButton />
          </header>
          <main className="min-h-0 w-full flex-1 overflow-y-auto p-7 max-[1000px]:p-[25px] max-[650px]:p-[18px]">
            <Outlet />
          </main>
        </div>
      </div>
      <Toaster
        theme={dark ? "dark" : "light"}
        position="top-center"
        duration={4000}
        closeButton
      />
      </TransactionShortcut>
    </ThemeContext.Provider>
  );
}

export function HomePage() {
  return (
    <>
      <div className="mb-[27px]">
        <p className="mb-2.5 text-[10px] font-bold tracking-[2px] text-brand">
          YOUR MONEY, YOUR HEYDAY
        </p>
        <h2 className="mb-[7px] text-[clamp(22px,2.5vw,30px)] font-[650] tracking-[-.9px]">
          A fresh start for your finances.
        </h2>
        <p className="text-[14px]">
          A little clarity today. More freedom tomorrow.
        </p>
      </div>
      <section className="grid min-h-[360px] grid-cols-2 items-center overflow-hidden rounded-[22px] border border-line bg-card p-9 max-[1000px]:p-[25px] max-[650px]:grid-cols-1">
        <div>
          <span className="inline-block rounded-[20px] bg-accent/20 px-3 py-1.5 text-[11px] font-semibold text-ink">
            Make yourself at home
          </span>
          <h2 className="mt-[18px] mb-3.5 text-[clamp(25px,3vw,39px)] leading-[1.2] font-[650] tracking-[-1.4px]">
            Your money.
            <br />
            All in one little world.
          </h2>
          <p className="max-w-[330px] text-[13px]">
            Bring your accounts and expected income together, and make room for
            what matters.
          </p>
          <div className="mt-6 flex flex-wrap gap-2.5">
            <Link
              to="/accounts"
              className="inline-flex items-center gap-[7px] rounded-[10px] bg-brand px-[15px] py-[11px] text-[12px] font-semibold text-white"
            >
              Explore accounts <ArrowUpRight size={17} />
            </Link>
            <Link
              to="/income"
              className="inline-flex items-center gap-[7px] rounded-[10px] border border-line px-[15px] py-[11px] text-[12px] font-semibold"
            >
              Explore income
            </Link>
          </div>
        </div>
        <img
          className="-ml-[4%] w-[115%] max-w-none dark:brightness-[.82] dark:saturate-[.85] max-[650px]:m-0 max-[650px]:w-full"
          src="/images/island.png"
          alt="A peaceful floating island with a purple-roofed home, trees, and a pond"
        />
      </section>
      <div className="mt-[22px] grid grid-cols-3 gap-[18px] max-[1000px]:grid-cols-1">
        <section className="rounded-[22px] border border-line bg-card p-[27px]">
          <Wallet className="mb-[19px] text-brand" size={23} />
          <h3 className="mb-[9px] text-[15px] font-[650]">Your accounts</h3>
          <p className="mb-[17px] text-[12px]">
            A home for your cash, savings, investments, and debts.
          </p>
          <Link to="/accounts" className="text-[12px] font-semibold text-brand">
            View accounts →
          </Link>
        </section>
        <section className="rounded-[22px] border border-line bg-card p-[27px]">
          <ArrowUpRight className="mb-[19px] text-brand" size={23} />
          <h3 className="mb-[9px] text-[15px] font-[650]">Expected income</h3>
          <p className="mb-[17px] text-[12px]">
            Plan around salary, variable income, and investment income.
          </p>
          <Link to="/income" className="text-[12px] font-semibold text-brand">
            View income →
          </Link>
        </section>
        <section className="rounded-[22px] border border-line bg-card p-[27px]">
          <SettingsIcon className="mb-[19px] text-brand" size={23} />
          <h3 className="mb-[9px] text-[15px] font-[650]">Your own rhythm</h3>
          <p className="mb-[17px] text-[12px]">
            A calendar month by default, with room for your payday cycle.
          </p>
          <Link to="/settings" className="text-[12px] font-semibold text-brand">
            View settings →
          </Link>
        </section>
      </div>
    </>
  );
}

export { AccountsPage } from "./components/AccountsPage";

export { IncomePage } from "./components/IncomePage";

export function SettingsPage() {
  const { dark, setDark } = useContext(ThemeContext);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [day, setDay] = useState(1);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [version, setVersion] = useState(appConfig.version);
  useEffect(() => {
    if (!desktopAvailable) return;
    let active = true;
    getVersion()
      .then((value) => {
        if (active) setVersion(value);
      })
      .catch(() => {
        /* Bundled config provides the fallback version. */
      });
    getSettings()
      .then((value) => {
        if (active) {
          setSettings(value);
          setDay(value.period_start_day);
        }
      })
      .catch(() => {
        if (active)
          setError("Could not load settings. Restart the app to try again.");
      });
    return () => {
      active = false;
    };
  }, []);

  async function savePeriod(event: React.FormEvent) {
    event.preventDefault();
    setSaving(true);
    setSaveError(null);
    try {
      const updated = await updatePeriod(day);
      setSettings(updated);
      setDay(updated.period_start_day);
      toast.success("Period saved.", { id: "period-saved" });
    } catch {
      setSaveError("Could not save your period. Please try again.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <div className="mb-6 max-w-[740px]">
        <h2 className="mb-3 text-[25px] font-[650]">Make Heyday yours.</h2>
        <p className="text-sm">Manage your preferences, payees, and spending categories.</p>
      </div>
      <Tabs defaultValue="general" className="max-w-[740px]">
        <TabsList aria-label="Settings sections">
          <TabsTrigger value="general">General</TabsTrigger>
          <TabsTrigger value="payees">Payees</TabsTrigger>
          <TabsTrigger value="categories">Categories</TabsTrigger>
        </TabsList>
        <TabsContent value="general" forceMount>
      <section className="rounded-[22px] border border-line bg-card p-[27px]">
        <h3 className="text-lg font-semibold">General</h3>
        <p className="mt-2 text-sm">Your theme, currency, and payday cycle apply across the app.</p>
        <div className="my-3 flex items-center justify-between gap-5 border-b border-line py-5 text-[14px]">
          <label htmlFor="theme">Theme</label>
          <select
            className="rounded-[9px] border border-line bg-page py-[9px] pr-8 pl-3 text-ink"
            id="theme"
            value={dark ? "dark" : "light"}
            onChange={(event) => setDark(event.target.value === "dark")}
          >
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </div>
        {!desktopAvailable ? (
          <p className="mt-2 rounded-xl bg-soft p-[18px] text-[14px]">
            Open the desktop app to access your local settings.
          </p>
        ) : error ? (
          <p className="mt-2 text-[14px]" role="alert">
            {error}
          </p>
        ) : !settings ? (
          <p className="mt-2 text-[14px]" role="status">
            Loading settings…
          </p>
        ) : (
          <>
            <CurrencySetting settings={settings} onChange={setSettings} />
            <form onSubmit={savePeriod}>
              <div className="my-3 flex items-center justify-between gap-5 border-b border-line py-5 text-[14px]">
                <label htmlFor="period-day">Period start day</label>
                <select
                  className="rounded-[9px] border border-line bg-page py-[9px] pr-8 pl-3 text-ink"
                  id="period-day"
                  value={day}
                  disabled={saving}
                  onChange={(event) => {
                    setDay(Number(event.target.value));
                    setSaveError(null);
                  }}
                >
                  {Array.from({ length: 31 }, (_, index) => index + 1).map(
                    (value) => (
                      <option key={value} value={value}>
                        {value === 1 ? "1 · Calendar month" : `Day ${value}`}
                      </option>
                    ),
                  )}
                </select>
              </div>
              <p className="mt-2 text-[14px]">
                Your current cycle: <strong>{periodLabel(day)}</strong>
              </p>
              <p className="mt-2 text-[14px]">
                If a month has fewer days, the cycle starts on its last day.
                Income schedules stay unchanged.
              </p>
              <div className="mt-4 flex flex-wrap gap-2.5">
                <button
                  className="inline-flex items-center gap-[7px] rounded-[10px] bg-brand px-[15px] py-[11px] text-[12px] font-semibold text-white"
                  type="submit"
                  disabled={saving || day === settings.period_start_day}
                >
                  {saving ? "Saving…" : "Save period"}
                </button>
              </div>
              {saveError && (
                <p className="mt-2 text-[14px]" role="alert">
                  {saveError}
                </p>
              )}
            </form>
            <dl className="my-[26px]">
              <div className="flex justify-between gap-5 border-b border-line py-[18px] text-[14px]">
                <dt>Currency</dt>
                <dd className="text-right text-muted">
                  {settings.currency ?? "Not selected yet"}
                </dd>
              </div>
              <div className="flex justify-between gap-5 border-b border-line py-[18px] text-[14px]">
                <dt>Storage</dt>
                <dd className="text-right text-muted">Local SQLite database</dd>
              </div>
            </dl>
          </>
        )}
        <p className="mt-2 text-[14px] font-normal text-muted">
          Backup/restore is planned.
        </p>
      </section>
          <DangerZone disabled={saving || !settings} />
        </TabsContent>
        <TabsContent value="payees" forceMount>
          {desktopAvailable ? <TransactionOptionsSettings kind="payee" /> : <p className="rounded-xl bg-soft p-5">Open the desktop app to manage payees.</p>}
        </TabsContent>
        <TabsContent value="categories" forceMount>
          {desktopAvailable ? <TransactionOptionsSettings kind="category" /> : <p className="rounded-xl bg-soft p-5">Open the desktop app to manage categories.</p>}
        </TabsContent>
      </Tabs>
      <p className="max-w-[740px] px-1 py-[18px] text-[12px]">
        Heyday Money · Version {version}
      </p>
    </>
  );
}
