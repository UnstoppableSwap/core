import { MenuItem, Select } from "@material-ui/core";
import { useAppSelector } from "store/hooks";
import TruncatedText from "renderer/components/other/TruncatedText";
import { PiconeroAmount } from "../../other/Units";
import { parseDateString } from "utils/parseUtils";

interface SwapSelectDropDownProps {
  selectedSwap: string | null;
  setSelectedSwap: (swapId: string | null) => void;
}

export default function SwapSelectDropDown({
  selectedSwap,
  setSelectedSwap,
}: SwapSelectDropDownProps) {
  const swaps = useAppSelector((state) =>
    Object.values(state.rpc.state.swapInfos),
  );

  return (
    <Select
      value={selectedSwap ?? ""}
      variant="outlined"
      onChange={(e) => setSelectedSwap(e.target.value as string || null)}
      style={{ width: "100%" }}
      displayEmpty
    >
      {swaps.map((swap) => (
          <MenuItem value={swap.swap_id} key={swap.swap_id}>
          Swap{" "}<TruncatedText>{swap.swap_id}</TruncatedText>{" "}from{" "}
          {new Date(parseDateString(swap.start_date)).toDateString()} (
          <PiconeroAmount amount={swap.xmr_amount} />)
        </MenuItem>
      ))}
      <MenuItem value="">Do not attach a swap</MenuItem>
    </Select>
  );
} 