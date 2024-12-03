import { Box, CircularProgress, Typography } from "@mui/material";
import makeStyles from '@mui/styles/makeStyles';
import { ReactNode } from "react";

const useStyles = makeStyles((theme) => ({
  subtitle: {
    paddingTop: theme.spacing(1),
  },
}));

export default function CircularProgressWithSubtitle({
  description,
}: {
  description: string | ReactNode;
}) {
  const classes = useStyles();

  return (
    <Box
      display="flex"
      justifyContent="center"
      alignItems="center"
      flexDirection="column"
    >
      <CircularProgress size={50} />
      <Typography variant="subtitle2" className={classes.subtitle}>
        {description}
      </Typography>
    </Box>
  );
}
