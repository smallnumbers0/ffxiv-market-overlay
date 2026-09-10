import { RefreshIcon } from "./icons";
import type { AppError, Board, Item, Listing, PriceData } from "../lib/tauriApi";
import { iconUrl } from "../lib/tauriApi";
import { gil, timeAgo, velocity } from "../lib/format";

/**
 * Cheapest listings shown per quality, per column. Five is enough to see
 * whether the bottom of a board is one lowball listing or a real floor, and
 * short enough that HQ and NQ both fit without scrolling.
 */
const LISTINGS_PER_QUALITY = 5;

/** What one column knows about its board's prices. */
export interface BoardPrice {
  price: PriceData | null;
  loading: boolean;
  error: AppError | null;
}

interface ComparisonPanelProps {
  item: Item;
  boards: Board[];
  prices: Record<string, BoardPrice | undefined>;
  onRefresh: () => void;
}

/**
 * One item, every board at once - a column per board, read left to right.
 *
 * Columns are deliberately narrower than the old single-board panel: Total and
 * World are dropped, because at two columns in a 460px overlay the numbers you
 * actually compare (price and quantity) are worth more than the ones you can
 * derive or infer.
 */
export function ComparisonPanel({
  item,
  boards,
  prices,
  onRefresh,
}: ComparisonPanelProps) {
  const icon = iconUrl(item.iconPath);
  const anyLoading = boards.some((board) => prices[board.id]?.loading);

  // Every column shows the same item at the same moment, so the freshest
  // timestamp among them speaks for the panel rather than each column
  // repeating its own.
  const fetchedAt = Math.max(
    0,
    ...boards.map((board) => prices[board.id]?.price?.fetchedAt ?? 0),
  );

  return (
    <section className="price-panel">
      {/* No back button: the search box stays on screen above this panel, so
          there is nothing to go back to - typing replaces what is here. */}
      <div className="price-header">
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
          disabled={anyLoading}
          title="Refresh every board"
          aria-label="Refresh every board"
        >
          <RefreshIcon />
        </button>
      </div>

      <div className="compare">
        {boards.map((board) => (
          <BoardColumn
            key={board.id}
            board={board}
            state={prices[board.id]}
            cheapest={isCheapest(board.id, boards, prices)}
          />
        ))}
      </div>

      {fetchedAt > 0 && (
        <p className="price-footer">fetched {timeAgo(fetchedAt)}</p>
      )}
    </section>
  );
}

/**
 * Whether this board has the lowest asking price on screen, so the column can
 * say so. Comparing the cheapest of either quality is what the user is
 * actually asking - "where do I buy this" - rather than NQ and HQ separately.
 *
 * Ties are not marked: with two boards at the same price there is nothing to
 * choose between them, and highlighting both says less than highlighting
 * neither.
 */
function isCheapest(
  id: string,
  boards: Board[],
  prices: Record<string, BoardPrice | undefined>,
): boolean {
  const best = (boardId: string): number | null => {
    const price = prices[boardId]?.price;
    if (!price) return null;
    const candidates = [price.minPriceNq, price.minPriceHq].filter(
      (value): value is number => value !== null,
    );
    return candidates.length > 0 ? Math.min(...candidates) : null;
  };

  const mine = best(id);
  if (mine === null) return false;
  const others = boards
    .filter((board) => board.id !== id)
    .map((board) => best(board.id))
    .filter((value): value is number => value !== null);
  return others.length > 0 && others.every((value) => mine < value);
}

function BoardColumn({
  board,
  state,
  cheapest,
}: {
  board: Board;
  state: BoardPrice | undefined;
  cheapest: boolean;
}) {
  const price = state?.price ?? null;
  const hq = price
    ? price.listings.filter((row) => row.hq).slice(0, LISTINGS_PER_QUALITY)
    : [];
  const nq = price
    ? price.listings.filter((row) => !row.hq).slice(0, LISTINGS_PER_QUALITY)
    : [];

  return (
    <div className="compare-column">
      <h3 className="compare-board" title={board.scope ?? undefined}>
        <span className="compare-board-name">{board.scope ?? "No board"}</span>
        {cheapest && (
          <span className="compare-flag" title="Cheapest board on screen">
            cheapest
          </span>
        )}
      </h3>

      {state?.error ? (
        <p className="error-message compare-note">{state.error.message}</p>
      ) : !price ? (
        <p className="compare-note">
          {state?.loading ? "Loading..." : "No price data yet."}
        </p>
      ) : (
        /* Numbers dim while a refresh is in flight rather than vanishing - a
           flash of empty is worse than a stale number that says it is stale. */
        <div className={`compare-body${state?.loading ? " is-stale" : ""}`}>
          <dl className="compare-stats">
            <Row label="HQ" value={gil(price.minPriceHq)} />
            <Row label="NQ" value={gil(price.minPriceNq)} />
            <Row label="Sales" value={velocity(price.saleVelocity)} />
          </dl>

          {/* Omitted rather than shown empty: most items have no HQ version
              at all, and a blank HQ block on every crystal is pure noise. */}
          {hq.length > 0 && <Listings title="Cheapest HQ" rows={hq} />}
          {nq.length > 0 ? (
            <Listings title="Cheapest NQ" rows={nq} />
          ) : (
            <p className="compare-note">Nothing listed.</p>
          )}
        </div>
      )}
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="compare-stat">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

/**
 * One quality's cheapest listings for a board.
 *
 * The world is what makes a data-center column actionable: "776 gil" is only
 * useful alongside where you would have to travel to pay it. Universalis omits
 * it on a single-world query, and there the column stands down to a blank
 * gutter rather than a row of dashes - it is still what soaks up the table's
 * slack width, keeping price and quantity clustered on the left.
 */
function Listings({ title, rows }: { title: string; rows: Listing[] }) {
  const showWorlds = rows.some((row) => row.worldName);
  return (
    <div className="compare-listings">
      <h4 className="compare-listings-title">{title}</h4>
      <table className="table compare-table">
        <tbody>
          {rows.map((row, index) => (
            <tr key={`${row.pricePerUnit}-${index}`}>
              {/* No HQ badge: the heading above already says which this is. */}
              <td className="price-cell">{gil(row.pricePerUnit)}</td>
              <td className="numeric compare-qty">&times;{row.quantity}</td>
              <td className="muted compare-world">
                {showWorlds ? (row.worldName ?? "-") : ""}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
