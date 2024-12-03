import { Box, List } from "@mui/material";
import AccountBalanceWalletIcon from "@mui/icons-material/AccountBalanceWallet";
import HelpOutlineIcon from "@mui/icons-material/HelpOutline";
import HistoryOutlinedIcon from "@mui/icons-material/HistoryOutlined";
import SwapHorizOutlinedIcon from "@mui/icons-material/SwapHorizOutlined";
import RouteListItemIconButton from "./RouteListItemIconButton";
import UnfinishedSwapsBadge from "./UnfinishedSwapsCountBadge";

export default function NavigationHeader() {
  return (
    <Box>
      <List>
        <RouteListItemIconButton name="Swap" route="/swap">
          <SwapHorizOutlinedIcon />
        </RouteListItemIconButton>
        <RouteListItemIconButton name="History" route="/history">
          <UnfinishedSwapsBadge>
            <HistoryOutlinedIcon />
          </UnfinishedSwapsBadge>
        </RouteListItemIconButton>
        <RouteListItemIconButton name="Wallet" route="/wallet">
          <AccountBalanceWalletIcon />
        </RouteListItemIconButton>
        <RouteListItemIconButton name="Help & Settings" route="/help">
          <HelpOutlineIcon />
        </RouteListItemIconButton>
      </List>
    </Box>
  );
}
