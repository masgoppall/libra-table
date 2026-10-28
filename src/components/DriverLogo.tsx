import { type Driver } from "@/types";

/** The round badge of a driver (connection list, picker, form title): the brand badges in
 * public/brand/drivers/badge, one per driver, named by its id. */
export function DriverCircle({ driver, size = 27 }: { driver: Driver; size?: number }) {
  return (
    <img
      className="driver-circle"
      src={`/brand/drivers/badge/${driver}.svg`}
      width={size}
      height={size}
      alt=""
      draggable={false}
    />
  );
}
