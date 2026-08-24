class SmsController < ApplicationController
  def create
    SmsStore.save(params[:body])
  end
end
