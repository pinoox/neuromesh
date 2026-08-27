import { type FC } from 'react';

export const StatCard: FC<{ label: string }> = ({ label }) => {
  return <div className="stat-card">{label}</div>;
};
