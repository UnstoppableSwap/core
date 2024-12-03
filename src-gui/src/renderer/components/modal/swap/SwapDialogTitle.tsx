import { Box, DialogTitle, Typography } from "@mui/material";
import makeStyles from '@mui/styles/makeStyles';
import DebugPageSwitchBadge from "./pages/DebugPageSwitchBadge";
import FeedbackSubmitBadge from "./pages/FeedbackSubmitBadge";
import TorStatusBadge from "./pages/TorStatusBadge";

const useStyles = makeStyles((theme) => ({
  root: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
  },
  rightSide: {
    display: "flex",
    alignItems: "center",
    gridGap: theme.spacing(1),
  },
}));

export default function SwapDialogTitle({
  title,
  debug,
  setDebug,
}: {
  title: string;
  debug: boolean;
  setDebug: (d: boolean) => void;
}) {
  const classes = useStyles();

  return (
    (<DialogTitle className={classes.root}>
      <Typography variant="h6">{title}</Typography>
      <Box className={classes.rightSide}>
        <FeedbackSubmitBadge />
        <DebugPageSwitchBadge enabled={debug} setEnabled={setDebug} />
        <TorStatusBadge />
      </Box>
    </DialogTitle>)
  );
}
