public class EnumFields {
    enum Coin {
        PENNY(1),
        NICKEL(5),
        DIME(10),
        QUARTER(25);

        private final int cents;

        Coin(int cents) {
            this.cents = cents;
        }

        int getCents() {
            return cents;
        }
    }

    public static void main(String[] args) {
        System.out.println(Coin.PENNY.getCents());
        System.out.println(Coin.QUARTER.getCents());
        System.out.println(Coin.DIME.name());
        System.out.println(Coin.values().length);
    }
}
