import { Box, Modal, makeStyles, IconButton } from "@material-ui/core";
import { useEffect, useState } from "react";
import { useAppSelector } from "store/hooks";
import { TauriContextStatusEvent } from "models/tauriModel";
import CliLogsBox from "../other/RenderedCliLog";
import { BackgroundProgressAlerts } from "../alert/DaemonStatusAlert";
import CloseIcon from "@material-ui/icons/Close";

const useStyles = makeStyles((theme) => ({
  modal: { display: "flex", alignItems: "center", justifyContent: "center" },
  container: {
    padding: theme.spacing(2),
    display: "flex",
    gap: theme.spacing(2),
    width: "95%",
    maxHeight: "90vh",
    position: "relative",
  },
  closeButton: {
    position: "absolute",
    top: theme.spacing(1),
    right: theme.spacing(1),
  },
  left: { flex: 0.3, overflowY: "auto", display: "flex", flexDirection: "column", gap: theme.spacing(1) },
  right: { flex: 0.7, overflowY: "auto" },
  progressBox: {
    backgroundColor: theme.palette.background.paper,
    padding: theme.spacing(1),
    borderRadius: theme.shape.borderRadius,
    marginBottom: theme.spacing(2),
    border: `1px solid ${theme.palette.divider}`,
    height: '100%',
    display: 'flex',
    flexDirection: 'column',
    gap: theme.spacing(1),
  },
}));

export default function ContextInitOverlay() {
  const classes = useStyles();
  const status = useAppSelector((s) => s.rpc.status);
  const logs = useAppSelector((s) => s.rpc.logs);
  const [overlayDismissed, setOverlayDismissed] = useState(false);

  const open = status === TauriContextStatusEvent.Initializing && !overlayDismissed;

  useEffect(() => {
    if (status && status !== TauriContextStatusEvent.Initializing) {
      setOverlayDismissed(true);
    }
  }, [status]);

  return (
    <Modal open={open} disableAutoFocus className={classes.modal}>
      <Box className={classes.container}>
        <IconButton className={classes.closeButton} onClick={() => { setOverlayDismissed(true); }}>
          <CloseIcon />
        </IconButton>
        <Box className={classes.left}>
          <Box className={classes.progressBox}>
            <BackgroundProgressAlerts />
          </Box>
        </Box>
        <Box className={classes.right}>
          <CliLogsBox label="Daemon Logs" logs={logs} autoScroll />
        </Box>
      </Box>
    </Modal>
  );
}
