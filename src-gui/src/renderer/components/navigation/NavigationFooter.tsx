import { Box } from "@mui/material";
import makeStyles from '@mui/styles/makeStyles';
import GitHubIcon from "@mui/icons-material/GitHub";
import RedditIcon from "@mui/icons-material/Reddit";
import DaemonStatusAlert from "../alert/DaemonStatusAlert";
import FundsLeftInWalletAlert from "../alert/FundsLeftInWalletAlert";
import MoneroWalletRpcUpdatingAlert from "../alert/MoneroWalletRpcUpdatingAlert";
import UnfinishedSwapsAlert from "../alert/UnfinishedSwapsAlert";
import DiscordIcon from "../icons/DiscordIcon";
import LinkIconButton from "../icons/LinkIconButton";
import { DISCORD_URL } from "../pages/help/ContactInfoBox";
import BackgroundRefundAlert from "../alert/BackgroundRefundAlert";

const useStyles = makeStyles((theme) => ({
  outer: {
    display: "flex",
    flexDirection: "column",
    padding: theme.spacing(1),
    gap: theme.spacing(1),
  },
  linksOuter: {
    display: "flex",
    justifyContent: "space-evenly",
  },
}));

export default function NavigationFooter() {
  const classes = useStyles();

  return (
    <Box className={classes.outer}>
      <FundsLeftInWalletAlert />
      <UnfinishedSwapsAlert />
      <BackgroundRefundAlert />
      <DaemonStatusAlert />
      <MoneroWalletRpcUpdatingAlert />
      <Box className={classes.linksOuter}>
        <LinkIconButton url="https://reddit.com/r/unstoppableswap">
          <RedditIcon />
        </LinkIconButton>
        <LinkIconButton url="https://github.com/UnstoppableSwap/unstoppableswap-gui">
          <GitHubIcon />
        </LinkIconButton>
        <LinkIconButton url={DISCORD_URL}>
          <DiscordIcon />
        </LinkIconButton>
      </Box>
    </Box>
  );
}
