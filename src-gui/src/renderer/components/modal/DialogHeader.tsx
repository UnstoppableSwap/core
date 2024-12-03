import { DialogTitle, Typography } from "@mui/material";
import makeStyles from '@mui/styles/makeStyles';
import { ReactNode } from "react";

const useStyles = makeStyles({
  root: {
    display: "flex",
    justifyContent: "space-between",
  },
});

type DialogTitleProps = {
  title: ReactNode;
};

export default function DialogHeader({ title }: DialogTitleProps) {
  const classes = useStyles();

  return (
    (<DialogTitle className={classes.root}>
      <Typography variant="h6">{title}</Typography>
    </DialogTitle>)
  );
}
