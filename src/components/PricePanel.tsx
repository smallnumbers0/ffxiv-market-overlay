import { BackIcon, RefreshIcon } from "./icons";
import type { AppError, Item, PriceData } from "../lib/tauriApi";
import { iconUrl } from "../lib/tauriApi";
import { gil, timeAgo, timeAgoSeconds, velocity } from "../lib/format";

interface PricePanelProps {
  item: Item;
  price: PriceData | null;
  loading: boolean;
  error: AppError | null;
  onBack: () => void;
  onRefresh: () => void;
}

export function PricePanel({
  item,
  price,
  loading,
  error,
  onBack,
  onRefresh,
}: PricePanelProps) {
  const icon = iconUrl(item.iconPath);

  return (
    <section className="price-panel">
      <div className="price-header">
        <button
          type="button"
          className="icon-button"
          onClick={onBack}
          title="Back to results (Esc)"
          aria-label="Back to results"
        >
          <BackIcon />
        </button>
        {icon && <img className="price-icon" src={icon} alt="" />}
        <div className="price-title">
          <h2>{item.name}</h2>
          <p className="price-subtitle">
            {[item.categoryName, item.levelItem ? `iLvl ${item.levelItem}` : null]
              .filter(Boolean)
              .join(" · ")}
          </p>
        </div>
        <button
          type="button"
          className="icon-button"
          onClick={onRefresh}
          disabled={loading}
          title="Refresh prices"
          aria-label="Refresh prices"
        >
          <RefreshIcon />
        </button>
      </div>

      {error && <ErrorState error={error} onRetry={onRefresh} />}

      {/* Keep the last-known numbers on screen while refreshing rather than
          blanking the panel - a flash of empty is worse than a stale number
          that is visibly labelled as being updated. */}
      {price && <PriceBody price={price} stale={loading} />}

      {!price && loading && <p className="empty">Loading prices...</p>}
      {!price && !loading && !error && (
        <p className="empty">No price data yet.</p>
      )}
    </section>
  );
}

function ErrorState({
  error,
  onRetry,
}: {
  error: AppError;
  onRetry: () => void;
}) {
  return (
    <div className="error-state" role="alert">
      <p className="error-message">{error.message}</p>
      {error.kind === "universalis" && (
        <p className="error-hint">
          Universalis may be down, or the network dropped. Your item catalog is
          local and still works.
        </p>
      )}
      <button type="button" className="button" onClick={onRetry}>
        Try again
      </button>
    </div>
  );
}

function PriceBody({ price, stale }: { price: PriceData; stale: boolean }) {
  const noData = price.listings.length === 0 && price.recentSales.length === 0;

  return (
    <div className={`price-body${stale ? " is-stale" : ""}`}>
      <dl className="stat-row">
        <Stat label="Cheapest NQ" value={gil(price.minPriceNq)} />
        <Stat label="Cheapest HQ" value={gil(price.minPriceHq)} />
        <Stat label="Avg NQ" value={gil(price.averagePriceNq)} />
        <Stat label="Avg HQ" value={gil(price.averagePriceHq)} />
        <Stat label="Sales" value={velocity(price.saleVelocity)} />
        <Stat
          label="For sale"
          value={price.unitsForSale === null ? "-" : String(price.unitsForSale)}
        />
      </dl>

      {noData ? (
        <p className="empty">
          Universalis has no data for this item on {price.scope} yet.
        </p>
      ) : (
        <>
          <Section title="Cheapest listings">
            {price.listings.length === 0 ? (
              <p className="empty">Nothing listed right now.</p>
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th className="price-cell">Price</th>
                    <th className="numeric">Qty</th>
                    <th className="numeric">Total</th>
                    <th>World</th>
                  </tr>
                </thead>
                <tbody>
                  {price.listings.slice(0, 8).map((listing, index) => (
                    <tr key={`${listing.pricePerUnit}-${index}`}>
                      <td className="price-cell">
                        {gil(listing.pricePerUnit)}
                        {listing.hq && <span className="hq-badge">HQ</span>}
                      </td>
                      <td className="numeric">{listing.quantity}</td>
                      <td className="numeric">{gil(listing.total)}</td>
                      <td className="muted">{listing.worldName ?? "-"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Section>

          <Section title="Recent sales">
            {price.recentSales.length === 0 ? (
              <p className="empty">No recorded sales.</p>
            ) : (
              <table className="table">
                <thead>
                  <tr>
                    <th className="price-cell">Price</th>
                    <th className="numeric">Qty</th>
                    <th>When</th>
                    <th>World</th>
                  </tr>
                </thead>
                <tbody>
                  {price.recentSales.slice(0, 8).map((sale, index) => (
                    <tr key={`${sale.timestamp}-${index}`}>
                      <td className="price-cell">
                        {gil(sale.pricePerUnit)}
                        {sale.hq && <span className="hq-badge">HQ</span>}
                      </td>
                      <td className="numeric">{sale.quantity}</td>
                      <td className="muted">{timeAgoSeconds(sale.timestamp)}</td>
                      <td className="muted">{sale.worldName ?? "-"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Section>
        </>
      )}

      <p className="price-footer">
        {price.scope} &middot; uploaded {timeAgo(price.lastUploadTime)} &middot;{" "}
        {stale ? "refreshing..." : `fetched ${timeAgo(price.fetchedAt)}`}
      </p>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="stat">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section className="section">
      <h3 className="section-title">{title}</h3>
      {children}
    </section>
  );
}
