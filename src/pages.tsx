import { getLocale } from "./lib/i18n"
import { LanguagePicker } from './components/LanguagePicker'
import { t as translate, useLanguage } from "./lib/i18n"
import { AppearancePicker } from "./components/AppearancePicker";
import { type Appearance, readAppearance } from "./lib/appearance";
import { Button } from "./components/ui/button";
import { NativeSelect } from "./components/ui/native-select";
import { PayeeLogoProvider } from "./components/PayeeLogo";
import { InstitutionProvider } from "./components/InstitutionProvider";
import { SubscriptionProvidersSettings } from "./components/SubscriptionProvidersSettings";
import { InstitutionsSettings } from "./components/InstitutionsSettings";
import { AppUpdates } from "./components/AppUpdates";
import { useLocalDate } from "./lib/useLocalDate";
import { DatabaseBackups } from "./components/DatabaseBackups";
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
  Users,
  Tags,
  Landmark,
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
  { to: "/", get label() { return translate("Home") }, icon: Home },
  { to: "/outlook", get label() { return translate("Outlook") }, icon: CalendarDays },
  { to: "/net-worth", get label() { return translate("Net Worth") }, icon: ChartNoAxesCombined },
  { to: "/transactions", get label() { return translate("Transactions") }, icon: ArrowLeftRight },
  { to: "/installments", get label() { return translate("Installments") }, icon: ListOrdered },
  { to: "/subscriptions", get label() { return translate("Subscriptions") }, icon: Repeat },
  { to: "/income", get label() { return translate("Income") }, icon: ArrowUpRight },
  { to: "/accounts", get label() { return translate("Accounts") }, icon: Wallet },
  { to: "/settings", get label() { return translate("Settings") }, icon: SettingsIcon },
] as const;

const ThemeContext = createContext({
  appearance: "heyday" as Appearance,
  setAppearance: (_appearance: Appearance) => {},
});

export function AppLayout() {
  useLanguage()

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
  const [appearance, setAppearance] = useState<Appearance>(readAppearance);
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const title =
    navigation.find((item) => item.to === pathname)?.label ?? (pathname.endsWith("/details") ? translate("Account details") : pathname.endsWith("/billing") ? translate("Credit card billing") : pathname.endsWith("/loans") ? translate("Borrowings & schedules") : pathname.startsWith("/accounts/") ? translate("Account transactions") : translate("Heyday"));

  useEffect(() => {
    savePreference("sidebar-collapsed", String(collapsed));
  }, [collapsed]);
  useEffect(() => {
    document.documentElement.dataset.theme = appearance;
    savePreference("theme", appearance);
  }, [appearance]);

  return (
    <ThemeContext.Provider value={{ appearance, setAppearance }}>
      <InstitutionProvider>
      <PayeeLogoProvider>
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
            className={`flex h-full min-h-0 min-w-0 flex-col overflow-hidden border-r border-line bg-card pt-[18px] pb-3 [&>*]:min-w-0 [&>*]:max-w-full ${collapsed ? "px-1" : "px-2.5"}`}
          >
            <Link
              to="/"
              className={`mb-[26px] flex shrink-0 items-center gap-[9px] text-[16px] font-[750] tracking-[-.8px] whitespace-nowrap max-[650px]:text-[14px] ${collapsed ? "w-full justify-center" : ""}`}
              aria-label={translate("Heyday Money home")}
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
              className="flex min-h-0 flex-1 flex-col gap-1.5"
              aria-label={translate("Main navigation")}
            >
              {navigation.map(({ to, label, icon: Icon }) => (
                <Fragment key={to}>
                  <Link
                    to={to}
                    activeOptions={{ exact: true }}
                    activeProps={{ "aria-current": "page" }}
                    className={`flex min-w-0 max-w-full shrink-0 items-center gap-2.5 rounded-[9px] px-2.5 py-2 text-[14px] font-medium text-muted hover:bg-page hover:text-ink aria-[current=page]:bg-soft aria-[current=page]:text-brand ${to === "/settings" ? "mt-auto" : ""} ${collapsed ? "justify-center" : ""}`}
                    title={collapsed ? translate(label) : undefined}
                    aria-label={translate(label)}
                  >
                    <Icon className="shrink-0" size={18} aria-hidden="true" />
                    {!collapsed && <span>{translate(label)}</span>}
                  </Link>
                  {to === "/accounts" && !collapsed && (
                    <div
                      role="region"
                      aria-label={translate("Saved accounts")}
                      tabIndex={0}
                      className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto overscroll-contain [scrollbar-width:none] [&::-webkit-scrollbar]:hidden focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-brand"
                    >
                      <SidebarAccounts />
                    </div>
                  )}
                </Fragment>
              ))}
            </nav>
            <button
              className={`mt-3 flex shrink-0 items-center gap-2.5 rounded-[9px] border-0 bg-transparent p-2.5 text-[12px] font-medium text-muted hover:bg-page hover:text-ink ${collapsed ? "justify-center" : ""}`}
              onClick={() => setCollapsed((value) => !value)}
              aria-expanded={!collapsed}
              aria-label={collapsed ? translate("Expand sidebar") : translate("Collapse sidebar")}
              title={collapsed ? translate("Expand sidebar") : translate("Collapse sidebar")}
            >
              {collapsed ? (
                <PanelLeftOpen className="shrink-0" size={18} />
              ) : (
                <>
                  <PanelLeftClose className="shrink-0" size={18} />
                  <span>{translate("Collapse sidebar")}</span>
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
            <h1 className="text-lg font-semibold">{translate(title)}</h1>
            <AddTransactionButton />
          </header>
          <main className="min-h-0 w-full flex-1 overflow-y-auto p-7 max-[1000px]:p-[25px] max-[650px]:p-[18px]">
            <Outlet />
          </main>
        </div>
      </div>
      <Toaster
        theme={appearance === "dark" ? "dark" : "light"}
        position="top-center"
        duration={4000}
        closeButton
      />
      </TransactionShortcut>
      </PayeeLogoProvider>
      </InstitutionProvider>
    </ThemeContext.Provider>
  );
}

export { HomePage } from "./components/HomePage";

export { AccountsPage } from "./components/AccountsPage";

export { IncomePage } from "./components/IncomePage";

export function SettingsPage() {
  useLanguage()

  const today = useLocalDate();
  const { appearance, setAppearance } = useContext(ThemeContext);
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
      toast.success(translate("Period saved."), { id: "period-saved" });
    } catch {
      setSaveError("Could not save your period. Please try again.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="@container mx-auto w-full max-w-[1080px]">
      <div className="mb-6 max-w-[740px]">
        <h2 className="mb-3 text-2xl font-semibold">{translate("Make Heyday yours.")}</h2>
        <p className="text-sm">{translate("Manage your preferences, institutions, payees, and spending categories.")}</p>
      </div>
      <Tabs defaultValue="general" orientation="vertical" className="grid min-w-0 grid-cols-1 items-stretch gap-0 overflow-clip rounded-2xl border border-line bg-card shadow-sm @min-[640px]:grid-cols-[220px_minmax(0,1fr)]">
        <TabsList aria-label={translate("Settings sections")} className="h-auto min-w-0 flex-col items-stretch justify-start gap-1 rounded-none border-b border-line bg-soft/50 p-3 @min-[640px]:border-r @min-[640px]:border-b-0">
          {([
            ['general', translate("General"), SettingsIcon],
            ['payees', translate("Payees"), Users],
            ['categories', translate("Categories"), Tags],
            ['institutions', translate("Institutions"), Landmark],
            ['providers', translate("Subscriptions"), Repeat],
          ] as const).map(([value, label, Icon]) => <TabsTrigger key={value} value={value} className="w-full flex-none justify-start gap-3 px-3 py-3 text-left">
            <Icon className="size-4 shrink-0" aria-hidden="true" /><span className="min-w-0 whitespace-normal break-normal">{label}</span>
          </TabsTrigger>)}
        </TabsList>
        <div className="min-w-0 p-4 [overflow-wrap:anywhere] @min-[640px]:p-6 @min-[900px]:p-8 [&_p]:leading-relaxed [&_fieldset]:min-w-0">
        <TabsContent value="providers"><SubscriptionProvidersSettings /></TabsContent>
        <TabsContent value="institutions"><InstitutionsSettings /></TabsContent>
        <TabsContent value="general" forceMount>
      <section className="min-w-0">
        <LanguagePicker />
        <AppearancePicker value={appearance} onChange={setAppearance} />
        {!desktopAvailable ? (
          <p className="mt-2 rounded-xl bg-soft p-[18px] text-[14px]">
            {translate("Open the desktop app to access your local settings.")}</p>
        ) : error ? (
          <p className="mt-2 text-[14px]" role="alert">
            {translate(error)}
          </p>
        ) : !settings ? (
          <p className="mt-2 text-[14px]" role="status">
            {translate("Loading settings…")}</p>
        ) : (
          <>
            <CurrencySetting settings={settings} onChange={setSettings} />
            <form className="border-b border-line pb-6" onSubmit={savePeriod}>
              <h3 className="text-base font-semibold">{translate("Payday cycle")}</h3>
              <p className="mt-1 text-xs">{translate("Group your dashboard by your monthly payday. Save to apply changes.")}</p>
              <div className="my-3 flex flex-wrap items-center justify-between gap-5 border-b border-line py-5 text-[14px]">
                <label htmlFor="period-day">{translate("Period start day")}</label>
                <div className="w-48 max-w-full"><NativeSelect
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
                        {value === 1 ? translate("1 · Calendar month") : translate("Day {value0}", { value0: value })}
                      </option>
                    ),
                  )}
                </NativeSelect></div>
              </div>
              <p className="mt-2 text-[14px]">
                {translate("Your current cycle:")}<strong>{periodLabel(day, today)}</strong>
              </p>
              <p className="mt-2 text-[14px]">
                {translate("If a month has fewer days, the cycle starts on its last day. Income schedules stay unchanged.")}</p>
              <div className="mt-4 flex flex-wrap gap-2.5">
                <Button
                  type="submit"
                  disabled={saving || day === settings.period_start_day}
                >
                  {saving ? translate("Saving…") : translate("Save period")}
                </Button>
              </div>
              {saveError && (
                <p className="mt-2 text-[14px]" role="alert">
                  {translate(saveError)}
                </p>
              )}
            </form>
            <h3 className="mt-6 text-base font-semibold">{translate("Storage & app information")}</h3>
            <dl className="my-3">
              <div className="flex flex-wrap justify-between gap-x-5 gap-y-2 border-b border-line py-[18px] text-[14px]">
                <dt>{translate("Currency")}</dt>
                <dd className="text-right text-muted">
                  {settings.currency ?? translate("Not selected yet")}
                </dd>
              </div>
              <div className="flex flex-wrap justify-between gap-x-5 gap-y-2 border-b border-line py-[18px] text-[14px]">
                <dt>{translate("Computer date")}</dt>
                <dd className="text-right text-muted">{today.toLocaleDateString(getLocale(), { day: "numeric", month: "short", year: "numeric" })}</dd>
              </div>
              <div className="flex flex-wrap justify-between gap-x-5 gap-y-2 border-b border-line py-[18px] text-[14px]">
                <dt>{translate("Storage")}</dt>
                <dd className="text-right text-muted">{translate("Local SQLite database")}</dd>
              </div>
            </dl>
          </>
        )}
      </section>
          <DatabaseBackups disabled={saving || !settings || day !== settings.period_start_day} />
          <AppUpdates version={version} disabled={saving || !!settings && day !== settings.period_start_day} />
          <DangerZone disabled={saving || !settings} />
        </TabsContent>
        <TabsContent value="payees" forceMount>
          {desktopAvailable ? <TransactionOptionsSettings kind="payee" /> : <p className="rounded-xl bg-soft p-5">{translate("Open the desktop app to manage payees.")}</p>}
        </TabsContent>
        <TabsContent value="categories" forceMount>
          {desktopAvailable ? <TransactionOptionsSettings kind="category" /> : <p className="rounded-xl bg-soft p-5">{translate("Open the desktop app to manage categories.")}</p>}
        </TabsContent>
          <p className="mt-6 border-t border-line pt-4 text-xs text-muted">
            {translate("Heyday Money · Version")} {version}
          </p>
        </div>
      </Tabs>
    </div>
  );
}
